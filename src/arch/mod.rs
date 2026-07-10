//! Architecture layer: raw syscall stubs, the [`Errno`] newtype, and the
//! `-errno` return-value decode. Everything else in the crate is built on top
//! of the `syscallN` functions re-exported here.

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!(
    "rusty_libc supports x86_64 and aarch64 only (see DESIGN.md). Keep using \
     the `libc` crate on other targets."
);

/// A positive `errno` value returned by a failing syscall.
///
/// The kernel signals errors by returning `-errno` in the `[-4095, -1]`
/// range; [`from_ret`] converts that into `Err(Errno(errno))` so a raw
/// negative value never escapes a wrapper. Interoperate with `std` via
/// `std::io::Error::from_raw_os_error(errno.0)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Errno(pub i32);

impl Errno {
    /// The bare positive `errno` integer.
    #[inline]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// The kernel returns error codes as `-errno` in this range; values outside it
/// are valid results (including large pointers/offsets reinterpreted as
/// `usize`).
const ERRNO_MAX: isize = 4095;

/// Decode a raw syscall return value into `Result<usize, Errno>`.
#[inline]
pub fn from_ret(ret: usize) -> Result<usize, Errno> {
    let signed = ret as isize;
    if (-ERRNO_MAX..0).contains(&signed) {
        Err(Errno(-signed as i32))
    } else {
        Ok(ret)
    }
}

/// Decode a raw syscall return value where success is any non-negative `i32`
/// (file descriptors, pid-like results). Errors map to [`Errno`].
#[inline]
pub fn from_ret_i32(ret: usize) -> Result<i32, Errno> {
    from_ret(ret).map(|v| v as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_errors_and_successes() {
        // -EPERM (1) .. -EINVAL (22) etc. decode to positive Errno.
        assert_eq!(from_ret((-1isize) as usize), Err(Errno(1)));
        assert_eq!(from_ret((-22isize) as usize), Err(Errno(22)));
        assert_eq!(from_ret((-4095isize) as usize), Err(Errno(4095)));
        // Just past the error window is a valid (huge) result, not an error.
        assert_eq!(from_ret((-4096isize) as usize), Ok((-4096isize) as usize));
        assert_eq!(from_ret(0), Ok(0));
        assert_eq!(from_ret(42), Ok(42));
    }
}
