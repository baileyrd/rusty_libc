//! Terminal attributes using the **kernel's** `struct termios` (`NCCS = 19`,
//! no glibc `c_ispeed`/`c_ospeed` fields), plus the controlling-terminal
//! foreground-group queries, `isatty`, and the [`Termios::make_raw`]
//! raw-mode recipe.
//!
//! Phase 1 provided the struct, `tcgetattr`/`tcsetattr`, `tcgetpgrp`/
//! `tcsetpgrp`, `isatty`, and the flag subset rush's line editor toggles.
//! Phase 2 adds the raw-mode transformation and validates the whole termios
//! path against a real terminal (see the PTY integration tests).

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
/// Ignore BREAK conditions on input.
pub const IGNBRK: u32 = 0o0000001;
/// Signal `SIGINT` on BREAK (else map to `\0` or discard).
pub const BRKINT: u32 = 0o0000002;
/// Mark parity/framing errors with a `\377 \0` prefix.
pub const PARMRK: u32 = 0o0000010;
/// Strip the eighth bit off input characters.
pub const ISTRIP: u32 = 0o0000040;
/// Map NL to CR on input.
pub const INLCR: u32 = 0o0000100;
/// Ignore CR on input.
pub const IGNCR: u32 = 0o0000200;
/// Map CR to NL on input.
pub const ICRNL: u32 = 0o0000400;
/// Enable start/stop output control.
pub const IXON: u32 = 0o0002000;

// Output flags (`c_oflag`).
/// Enable implementation-defined output processing.
pub const OPOST: u32 = 0o0000001;

// Control flags (`c_cflag`).
/// Character-size mask.
pub const CSIZE: u32 = 0o0000060;
/// 8 bits per character.
pub const CS8: u32 = 0o0000060;
/// Enable parity generation on output and checking on input.
pub const PARENB: u32 = 0o0000400;

// Local flags (`c_lflag`).
/// Enable signals (`INTR`, `QUIT`, `SUSP`).
pub const ISIG: u32 = 0o0000001;
/// Canonical (line-buffered) input.
pub const ICANON: u32 = 0o0000002;
/// Echo input characters.
pub const ECHO: u32 = 0o0000010;
/// Echo NL even when `ECHO` is off.
pub const ECHONL: u32 = 0o0000100;
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

impl Termios {
    /// Transform these attributes into "raw" mode in place: no canonical line
    /// processing, no echo, no signal generation, no CR/NL or output
    /// translation, 8-bit characters, and byte-at-a-time reads (`VMIN = 1`,
    /// `VTIME = 0`).
    ///
    /// This is the canonical `cfmakeraw(3)` recipe. The typical use is
    /// `let saved = tcgetattr(fd)?; let mut raw = saved; raw.make_raw();
    /// tcsetattr(fd, &raw)?;` then restore `saved` on exit. Callers that need
    /// finer control can manipulate the flag fields directly with the `pub`
    /// constants in this module.
    pub fn make_raw(&mut self) {
        self.c_iflag &= !(IGNBRK | BRKINT | PARMRK | ISTRIP | INLCR | IGNCR | ICRNL | IXON);
        self.c_oflag &= !OPOST;
        self.c_lflag &= !(ECHO | ECHONL | ICANON | ISIG | IEXTEN);
        self.c_cflag &= !(CSIZE | PARENB);
        self.c_cflag |= CS8;
        self.c_cc[VMIN] = 1;
        self.c_cc[VTIME] = 0;
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

    #[test]
    fn make_raw_clears_the_expected_flags() {
        // Start from a "cooked" terminal with the interesting bits set.
        let mut t = Termios {
            c_iflag: ICRNL | IXON | ISTRIP | BRKINT,
            c_oflag: OPOST,
            c_lflag: ICANON | ECHO | ISIG | IEXTEN | ECHONL,
            c_cflag: PARENB, // and CSIZE bits absent → make_raw must set CS8
            ..Default::default()
        };
        t.c_cc[VMIN] = 0;
        t.c_cc[VTIME] = 5;

        t.make_raw();

        // Every processing flag cleared.
        assert_eq!(t.c_iflag, 0);
        assert_eq!(t.c_oflag & OPOST, 0);
        assert_eq!(t.c_lflag & (ICANON | ECHO | ISIG | IEXTEN | ECHONL), 0);
        // 8-bit, no parity.
        assert_eq!(t.c_cflag & PARENB, 0);
        assert_eq!(t.c_cflag & CSIZE, CS8);
        // Byte-at-a-time, no timeout.
        assert_eq!(t.c_cc[VMIN], 1);
        assert_eq!(t.c_cc[VTIME], 0);
    }

    // --- PTY integration: validate the real-terminal path, not just the
    // ENOTTY-on-a-pipe path. ---

    // ioctls used only to allocate a pty pair from /dev/ptmx.
    const TIOCSPTLCK: usize = 0x4004_5431; // unlock the slave
    const TIOCGPTN: usize = 0x8004_5430; // fetch the slave's number

    /// Open a `(master, slave)` pty pair as owned `File`s so the fds close on
    /// drop. Uses std only to `open(2)`; every terminal op under test goes
    /// through this crate's wrappers.
    fn open_pty() -> (std::fs::File, std::fs::File) {
        use std::os::fd::AsRawFd;

        let master = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/ptmx")
            .expect("open /dev/ptmx");
        let mfd = master.as_raw_fd();

        // Unlock the slave (TIOCSPTLCK with a zero lock value).
        let unlock: i32 = 0;
        unsafe { ioctl(mfd, TIOCSPTLCK, &unlock as *const i32 as usize) }.expect("TIOCSPTLCK");

        // Discover the slave device number.
        let mut ptn: i32 = 0;
        unsafe { ioctl(mfd, TIOCGPTN, &mut ptn as *mut i32 as usize) }.expect("TIOCGPTN");

        let slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/pts/{ptn}"))
            .expect("open slave pts");

        (master, slave)
    }

    #[test]
    fn pty_slave_is_a_tty() {
        use std::os::fd::AsRawFd;
        let (_m, s) = open_pty();
        assert!(isatty(s.as_raw_fd()));
        assert!(tcgetattr(s.as_raw_fd()).is_ok());
    }

    #[test]
    fn raw_mode_roundtrips_through_the_kernel() {
        use std::os::fd::AsRawFd;
        let (_m, s) = open_pty();
        let sfd = s.as_raw_fd();

        // A fresh pty slave comes up in cooked mode: ICANON and ECHO set.
        let cooked = tcgetattr(sfd).expect("tcgetattr");
        assert_ne!(cooked.c_lflag & ICANON, 0);
        assert_ne!(cooked.c_lflag & ECHO, 0);

        // Apply raw mode and read it back from the kernel.
        let mut raw = cooked;
        raw.make_raw();
        tcsetattr(sfd, &raw).expect("tcsetattr raw");

        let after = tcgetattr(sfd).expect("tcgetattr after");
        assert_eq!(after.c_lflag & (ICANON | ECHO | ISIG | IEXTEN), 0);
        assert_eq!(after.c_iflag & (ICRNL | IXON), 0);
        assert_eq!(after.c_cc[VMIN], 1);
        assert_eq!(after.c_cc[VTIME], 0);

        // Restore the saved attributes and confirm the round-trip.
        tcsetattr(sfd, &cooked).expect("tcsetattr restore");
        let restored = tcgetattr(sfd).expect("tcgetattr restored");
        assert_ne!(restored.c_lflag & ICANON, 0);
        assert_ne!(restored.c_lflag & ECHO, 0);
    }

    #[test]
    fn tcsetpgrp_tcgetpgrp_roundtrip_on_pty() {
        use std::os::fd::AsRawFd;
        let (_m, s) = open_pty();
        let sfd = s.as_raw_fd();

        // Set the slave's foreground group to our own and read it back.
        // Without a controlling terminal the kernel may reject the set
        // (ENOTTY/EPERM); that is a valid, non-panicking outcome, so only
        // assert the round-trip when the set succeeds.
        let pgrp = crate::process::getpid();
        if tcsetpgrp(sfd, pgrp).is_ok() {
            assert_eq!(tcgetpgrp(sfd).expect("tcgetpgrp"), pgrp);
        }
    }
}
