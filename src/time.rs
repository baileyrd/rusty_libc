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
pub fn clock_gettime(clockid: i32) -> Result<Timespec, Errno> {
    let mut ts = Timespec::default();

    // Fast path: the vDSO reads the clock without entering the kernel. A
    // non-zero return means the vDSO declined (e.g. an unsupported clock), so
    // we fall through to the syscall, which reproduces success or the error.
    if let Some(f) = crate::vdso::clock_gettime_fn() {
        // SAFETY: `f` is the resolved vDSO entry with exactly this ABI; `ts` is
        // a valid, exclusively-borrowed `timespec` it writes on success.
        if unsafe { f(clockid, &mut ts as *mut Timespec) } == 0 {
            return Ok(ts);
        }
    }

    // clock_gettime(clockid, &mut ts).
    // SAFETY: `ts` is a valid, exclusively-borrowed `struct timespec` the
    // kernel writes.
    let ret = unsafe {
        syscall2(
            nr::CLOCK_GETTIME,
            clockid as usize,
            &mut ts as *mut Timespec as usize,
        )
    };
    from_ret(ret)?;
    Ok(ts)
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
