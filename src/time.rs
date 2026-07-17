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
pub fn clock_gettime(clockid: i32) -> Result<Timespec, Errno> {
    let mut ts = Timespec::default();
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
