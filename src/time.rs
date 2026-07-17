//! Clocks and sleeping: `clock_gettime` and `nanosleep`.

use crate::arch::nr;
use crate::arch::{from_ret, syscall2, Errno};

/// A time value with nanosecond resolution (kernel `struct timespec`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timespec {
    /// Whole seconds.
    pub tv_sec: i64,
    /// Nanoseconds within the second (`0..1_000_000_000`).
    pub tv_nsec: i64,
}

const _: () = assert!(core::mem::size_of::<Timespec>() == 16);

/// Wall-clock time since the Unix epoch (settable; can jump).
pub const CLOCK_REALTIME: i32 = 0;
/// Monotonic time since an unspecified start; never jumps backward. The right
/// clock for measuring elapsed intervals (timeouts, `time`-ing a command).
pub const CLOCK_MONOTONIC: i32 = 1;
/// Monotonic time that also advances while the system is suspended (unlike
/// [`CLOCK_MONOTONIC`]).
pub const CLOCK_BOOTTIME: i32 = 7;
/// A faster, lower-resolution [`CLOCK_REALTIME`]: on the vDSO fast path this
/// skips the fine-grained interpolation step, at the cost of coarser precision
/// (typically 1–4 ms, one kernel timer tick). Good for a prompt timestamp or
/// any read where sub-millisecond accuracy is not needed — it is roughly 2–3x
/// faster than the precise clock (see `bench/`).
pub const CLOCK_REALTIME_COARSE: i32 = 5;
/// A faster, lower-resolution [`CLOCK_MONOTONIC`]; see [`CLOCK_REALTIME_COARSE`]
/// for the precision/speed trade-off. The right choice for `$SECONDS`-style
/// counters, throttling, or any hot-path timestamp that only needs
/// millisecond-ish accuracy.
pub const CLOCK_MONOTONIC_COARSE: i32 = 6;

impl Timespec {
    /// Construct from a whole number of milliseconds.
    #[inline]
    pub const fn from_millis(ms: i64) -> Self {
        Timespec {
            tv_sec: ms / 1000,
            tv_nsec: (ms % 1000) * 1_000_000,
        }
    }
}

/// Read the current value of clock `clockid` (a `CLOCK_*` constant).
///
/// When the process vDSO exports a usable `clock_gettime`, this reads the clock
/// entirely in userspace (no syscall trap), matching glibc's speed; otherwise
/// it falls back to the raw syscall. The result is identical either way.
#[inline]
pub fn clock_gettime(clockid: i32) -> Result<Timespec, Errno> {
    // Fast path: the vDSO reads the clock without entering the kernel. Left
    // uninitialized until written: both the vDSO entry and the syscall below
    // fill it completely on success, and neither path reads it first.
    let mut ts = core::mem::MaybeUninit::<Timespec>::uninit();

    // A non-zero return means the vDSO declined (e.g. an unsupported clock),
    // so we fall through to the syscall, which reproduces success or the error.
    if let Some(f) = crate::vdso::clock_gettime_fn() {
        // SAFETY: `f` is the resolved vDSO entry with exactly this ABI; `ts` is
        // a valid, exclusively-borrowed, suitably-sized-and-aligned
        // `timespec` it writes on success.
        if unsafe { f(clockid, ts.as_mut_ptr()) } == 0 {
            // SAFETY: a zero return means the vDSO fully initialized `ts`.
            return Ok(unsafe { ts.assume_init() });
        }
    }

    // clock_gettime(clockid, &mut ts).
    // SAFETY: `ts` is a valid, exclusively-borrowed, suitably-sized-and-aligned
    // `struct timespec`; the kernel writes it completely on success and this
    // crate's `no_std` syscall path never reads it before checking `from_ret`.
    let ret = unsafe {
        syscall2(
            nr::CLOCK_GETTIME,
            clockid as usize,
            ts.as_mut_ptr() as usize,
        )
    };
    from_ret(ret)?;
    // SAFETY: `from_ret` returned `Ok`, so the kernel fully initialized `ts`.
    Ok(unsafe { ts.assume_init() })
}

/// Suspend execution for at least the interval `req`.
///
/// If a signal interrupts the sleep, this returns `Err(EINTR)`; when `rem` is
/// `Some`, the time left unslept is written there so the caller can resume.
pub fn nanosleep(req: &Timespec, rem: Option<&mut Timespec>) -> Result<(), Errno> {
    let rem_ptr = match rem {
        Some(r) => r as *mut Timespec as usize,
        None => 0,
    };
    // nanosleep(req, rem).
    // SAFETY: `req` is a valid `*const timespec` the kernel reads; `rem` is null
    // or a valid `*mut timespec` the kernel writes on EINTR.
    let ret = unsafe { syscall2(nr::NANOSLEEP, req as *const Timespec as usize, rem_ptr) };
    from_ret(ret).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_clock_advances() {
        let a = clock_gettime(CLOCK_MONOTONIC).expect("clock_gettime");
        // Sleep a short, bounded interval, then confirm the clock moved forward.
        nanosleep(&Timespec::from_millis(5), None).expect("nanosleep");
        let b = clock_gettime(CLOCK_MONOTONIC).expect("clock_gettime");
        let delta_ns = (b.tv_sec - a.tv_sec) * 1_000_000_000 + (b.tv_nsec - a.tv_nsec);
        assert!(delta_ns >= 5_000_000, "monotonic clock did not advance 5ms");
    }

    #[test]
    fn realtime_is_after_2020() {
        // Sanity: the realtime clock is past 2020-01-01 (1_577_836_800).
        let now = clock_gettime(CLOCK_REALTIME).expect("clock_gettime");
        assert!(now.tv_sec > 1_577_836_800);
    }

    #[test]
    fn coarse_and_boottime_clocks_work() {
        // Coarse clocks trade resolution for speed but report the same epoch.
        let realtime_coarse = clock_gettime(CLOCK_REALTIME_COARSE).expect("clock_gettime");
        assert!(realtime_coarse.tv_sec > 1_577_836_800);

        // Coarse monotonic must also advance, like the precise clock.
        let a = clock_gettime(CLOCK_MONOTONIC_COARSE).expect("clock_gettime");
        nanosleep(&Timespec::from_millis(20), None).expect("nanosleep");
        let b = clock_gettime(CLOCK_MONOTONIC_COARSE).expect("clock_gettime");
        let delta_ns = (b.tv_sec - a.tv_sec) * 1_000_000_000 + (b.tv_nsec - a.tv_nsec);
        assert!(delta_ns > 0, "coarse monotonic clock did not advance");

        // BOOTTIME tracks MONOTONIC closely absent a suspend/resume in between.
        let boot = clock_gettime(CLOCK_BOOTTIME).expect("clock_gettime");
        assert!(boot.tv_sec > 0);
    }

    // Read CLOCK_MONOTONIC directly via the syscall, bypassing the vDSO.
    fn raw_monotonic() -> i128 {
        let mut ts = Timespec::default();
        let ret = unsafe {
            crate::arch::syscall2(
                nr::CLOCK_GETTIME,
                CLOCK_MONOTONIC as usize,
                &mut ts as *mut Timespec as usize,
            )
        };
        crate::arch::from_ret(ret).expect("raw clock_gettime");
        ts.tv_sec as i128 * 1_000_000_000 + ts.tv_nsec as i128
    }

    #[test]
    fn vdso_wrapper_agrees_with_raw_syscall() {
        // Interleave raw-syscall reads around the public (maybe-vDSO) read; on
        // the monotonic clock the three must be non-decreasing. A vDSO entry
        // that returned garbage would violate the ordering.
        let before = raw_monotonic();
        let mid = clock_gettime(CLOCK_MONOTONIC).expect("clock_gettime");
        let mid_ns = mid.tv_sec as i128 * 1_000_000_000 + mid.tv_nsec as i128;
        let after = raw_monotonic();

        assert!(
            before <= mid_ns,
            "wrapper read is before the prior syscall read"
        );
        assert!(
            mid_ns <= after,
            "wrapper read is after the later syscall read"
        );
    }

    // The vDSO is always mapped on native x86_64 (and readable via
    // /proc/self/auxv), so resolution must succeed there — proving the fast
    // path is actually taken rather than silently always falling back. Not
    // asserted on aarch64, where the CI job runs under qemu-user, which may not
    // expose a guest vDSO.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn vdso_resolves_on_native_x86_64() {
        assert!(
            crate::vdso::clock_gettime_fn().is_some(),
            "vDSO clock_gettime should resolve on native x86_64"
        );
    }

    #[test]
    fn from_millis_splits_correctly() {
        assert_eq!(
            Timespec::from_millis(1500),
            Timespec {
                tv_sec: 1,
                tv_nsec: 500_000_000
            }
        );
        assert_eq!(
            Timespec::from_millis(0),
            Timespec {
                tv_sec: 0,
                tv_nsec: 0
            }
        );
    }
}
