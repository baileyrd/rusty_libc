//! Terminal attributes using the **kernel's** `struct termios` (`NCCS = 19`,
//! no glibc `c_ispeed`/`c_ospeed` fields), plus the controlling-terminal
//! foreground-group queries and `isatty`.
//!
//! Phase 1 provides the struct, `tcgetattr`/`tcsetattr`, `tcgetpgrp`/
//! `tcsetpgrp`, and `isatty`; the flag constants below are the subset rush's
//! line editor toggles. Raw-mode validation against rush's PTY suite is Phase
//! 2.

use crate::arch::Errno;
use crate::fd::ioctl;

/// Number of control characters in the kernel `struct termios`.
pub const NCCS: usize = 19;

// ioctl requests (asm-generic / x86_64).
const TCGETS: usize = 0x5401;
const TCSETSW: usize = 0x5403; // TCSADRAIN semantics: drain output, then set.
const TIOCGPGRP: usize = 0x540f;
const TIOCSPGRP: usize = 0x5410;

// Input flags (`c_iflag`).
/// Map CR to NL on input.
pub const ICRNL: u32 = 0o0000400;
/// Map NL to CR on input.
pub const INLCR: u32 = 0o0000100;
/// Enable start/stop output control.
pub const IXON: u32 = 0o0002000;

// Local flags (`c_lflag`).
/// Enable signals (`INTR`, `QUIT`, `SUSP`).
pub const ISIG: u32 = 0o0000001;
/// Canonical (line-buffered) input.
pub const ICANON: u32 = 0o0000002;
/// Echo input characters.
pub const ECHO: u32 = 0o0000010;
/// Enable implementation-defined input processing.
pub const IEXTEN: u32 = 0o0100000;

// Indices into `c_cc`.
/// `c_cc` index: minimum bytes for a non-canonical read.
pub const VMIN: usize = 6;
/// `c_cc` index: read timeout (tenths of a second) for non-canonical reads.
pub const VTIME: usize = 5;

/// Kernel `struct termios`. Field order and sizes match `<asm/termbits.h>`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Termios {
    /// Input mode flags.
    pub c_iflag: u32,
    /// Output mode flags.
    pub c_oflag: u32,
    /// Control mode flags.
    pub c_cflag: u32,
    /// Local mode flags.
    pub c_lflag: u32,
    /// Line discipline.
    pub c_line: u8,
    /// Control characters, indexed by the `V*` constants.
    pub c_cc: [u8; NCCS],
}

const _: () = assert!(core::mem::size_of::<Termios>() == 36);
const _: () = assert!(core::mem::offset_of!(Termios, c_iflag) == 0);
const _: () = assert!(core::mem::offset_of!(Termios, c_oflag) == 4);
const _: () = assert!(core::mem::offset_of!(Termios, c_cflag) == 8);
const _: () = assert!(core::mem::offset_of!(Termios, c_lflag) == 12);
const _: () = assert!(core::mem::offset_of!(Termios, c_line) == 16);
const _: () = assert!(core::mem::offset_of!(Termios, c_cc) == 17);

impl Default for Termios {
    fn default() -> Self {
        Termios {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; NCCS],
        }
    }
}

/// Read the terminal attributes of `fd`.
pub fn tcgetattr(fd: i32) -> Result<Termios, Errno> {
    let mut t = Termios::default();
    // SAFETY: `TCGETS` expects a `*mut termios`; `t` is exactly that.
    unsafe { ioctl(fd, TCGETS, &mut t as *mut Termios as usize) }?;
    Ok(t)
}

/// Set the terminal attributes of `fd` after draining pending output
/// (`TCSADRAIN`).
pub fn tcsetattr(fd: i32, termios: &Termios) -> Result<(), Errno> {
    // SAFETY: `TCSETSW` expects a `*const termios`; `termios` is a valid
    // borrow the kernel only reads.
    unsafe { ioctl(fd, TCSETSW, termios as *const Termios as usize) }?;
    Ok(())
}

/// Get the foreground process group ID of the terminal `fd`.
pub fn tcgetpgrp(fd: i32) -> Result<i32, Errno> {
    let mut pgrp: i32 = 0;
    // SAFETY: `TIOCGPGRP` expects a `*mut pid_t`.
    unsafe { ioctl(fd, TIOCGPGRP, &mut pgrp as *mut i32 as usize) }?;
    Ok(pgrp)
}

/// Set the foreground process group of the terminal `fd` to `pgrp`.
pub fn tcsetpgrp(fd: i32, pgrp: i32) -> Result<(), Errno> {
    // SAFETY: `TIOCSPGRP` expects a `*const pid_t` the kernel only reads.
    unsafe { ioctl(fd, TIOCSPGRP, &pgrp as *const i32 as usize) }?;
    Ok(())
}

/// Return `true` if `fd` refers to a terminal (a successful `TCGETS`).
pub fn isatty(fd: i32) -> bool {
    tcgetattr(fd).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_is_not_a_tty() {
        let (r, w) = crate::fd::pipe2(0).unwrap();
        assert!(!isatty(r));
        assert_eq!(tcgetattr(r), Err(Errno(25))); // ENOTTY
        crate::fd::close(r).unwrap();
        crate::fd::close(w).unwrap();
    }

    #[test]
    fn layout_is_kernel_not_glibc() {
        // glibc's termios is 60 bytes (NCCS=32 + speed fields); the kernel's
        // is 36. The const assertions above already enforce this at compile
        // time; restate it as a runtime guard for clarity.
        assert_eq!(core::mem::size_of::<Termios>(), 36);
        assert_eq!(NCCS, 19);
    }
}
