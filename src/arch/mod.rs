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

    /// The symbolic name (`"EBADF"`, `"EINVAL"`, …) for the common Linux
    /// `errno` values, or `None` for a code this crate does not name.
    pub const fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            1 => "EPERM",
            2 => "ENOENT",
            3 => "ESRCH",
            4 => "EINTR",
            5 => "EIO",
            6 => "ENXIO",
            7 => "E2BIG",
            8 => "ENOEXEC",
            9 => "EBADF",
            10 => "ECHILD",
            11 => "EAGAIN",
            12 => "ENOMEM",
            13 => "EACCES",
            14 => "EFAULT",
            16 => "EBUSY",
            17 => "EEXIST",
            18 => "EXDEV",
            19 => "ENODEV",
            20 => "ENOTDIR",
            21 => "EISDIR",
            22 => "EINVAL",
            23 => "ENFILE",
            24 => "EMFILE",
            25 => "ENOTTY",
            26 => "ETXTBSY",
            27 => "EFBIG",
            28 => "ENOSPC",
            29 => "ESPIPE",
            30 => "EROFS",
            31 => "EMLINK",
            32 => "EPIPE",
            33 => "EDOM",
            34 => "ERANGE",
            36 => "ENAMETOOLONG",
            38 => "ENOSYS",
            39 => "ENOTEMPTY",
            40 => "ELOOP",
            110 => "ETIMEDOUT",
            111 => "ECONNREFUSED",
            _ => return None,
        })
    }
}

/// Named constants for the `errno` values callers most often match on. The
/// numeric codes are the Linux `asm-generic` values, identical on x86_64 and
/// aarch64.
impl Errno {
    /// Operation not permitted.
    pub const EPERM: Errno = Errno(1);
    /// No such file or directory.
    pub const ENOENT: Errno = Errno(2);
    /// No such process.
    pub const ESRCH: Errno = Errno(3);
    /// Interrupted system call.
    pub const EINTR: Errno = Errno(4);
    /// I/O error.
    pub const EIO: Errno = Errno(5);
    /// Bad file descriptor.
    pub const EBADF: Errno = Errno(9);
    /// No child processes.
    pub const ECHILD: Errno = Errno(10);
    /// Resource temporarily unavailable (a.k.a. `EWOULDBLOCK`).
    pub const EAGAIN: Errno = Errno(11);
    /// Out of memory.
    pub const ENOMEM: Errno = Errno(12);
    /// Permission denied.
    pub const EACCES: Errno = Errno(13);
    /// Bad address.
    pub const EFAULT: Errno = Errno(14);
    /// Device or resource busy.
    pub const EBUSY: Errno = Errno(16);
    /// File exists.
    pub const EEXIST: Errno = Errno(17);
    /// Not a directory.
    pub const ENOTDIR: Errno = Errno(20);
    /// Is a directory.
    pub const EISDIR: Errno = Errno(21);
    /// Invalid argument.
    pub const EINVAL: Errno = Errno(22);
    /// Too many open files.
    pub const EMFILE: Errno = Errno(24);
    /// Not a typewriter (not a terminal).
    pub const ENOTTY: Errno = Errno(25);
    /// Illegal seek.
    pub const ESPIPE: Errno = Errno(29);
    /// Broken pipe.
    pub const EPIPE: Errno = Errno(32);
    /// Function not implemented.
    pub const ENOSYS: Errno = Errno(38);
}

impl core::fmt::Display for Errno {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} (os error {})", self.0),
            None => write!(f, "os error {}", self.0),
        }
    }
}

impl core::error::Error for Errno {}

/// Convert an [`Errno`] into a `std::io::Error` (feature `std`). This is the
/// bridge for callers that work in terms of `std::io::Result`.
#[cfg(any(feature = "std", test))]
impl From<Errno> for std::io::Error {
    fn from(e: Errno) -> Self {
        std::io::Error::from_raw_os_error(e.0)
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

    #[test]
    fn named_constants_match_codes() {
        assert_eq!(Errno::EBADF, Errno(9));
        assert_eq!(Errno::EINVAL, Errno(22));
        assert_eq!(Errno::ENOTTY, Errno(25));
        assert_eq!(Errno::ECHILD, Errno(10));
        assert_eq!(Errno::EBADF.code(), 9);
    }

    #[test]
    fn display_and_name() {
        assert_eq!(Errno(9).name(), Some("EBADF"));
        assert_eq!(Errno(4095).name(), None);
        assert_eq!(std::format!("{}", Errno(9)), "EBADF (os error 9)");
        assert_eq!(std::format!("{}", Errno(4095)), "os error 4095");
    }

    #[test]
    fn converts_to_io_error() {
        let io: std::io::Error = Errno::EBADF.into();
        assert_eq!(io.raw_os_error(), Some(9));
    }
}
