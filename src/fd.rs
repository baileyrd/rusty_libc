//! File-descriptor primitives: `read`, `poll`, `pipe2`, `dup`/`dup2`, `close`,
//! `fcntl`, and the crate-internal `ioctl` shim shared by [`crate::termios`]
//! and [`crate::tty`].

use crate::arch::nr;
use crate::arch::{from_ret, from_ret_i32, syscall1, syscall2, syscall3, Errno};

/// `poll(2)` event/return flag: data available to read.
pub const POLLIN: i16 = 0x001;

/// `fcntl(2)` command: get the file-descriptor flags.
pub const F_GETFD: i32 = 1;
/// `fcntl(2)` command: set the file-descriptor flags.
pub const F_SETFD: i32 = 2;
/// File-descriptor flag: close the fd on `execve`.
pub const FD_CLOEXEC: i32 = 1;

/// A `poll(2)` request/response entry. Kernel `struct pollfd` layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PollFd {
    /// File descriptor to watch (negative fds are ignored by the kernel).
    pub fd: i32,
    /// Requested events (e.g. [`POLLIN`]).
    pub events: i16,
    /// Events that actually occurred, filled in by the kernel.
    pub revents: i16,
}

const _: () = assert!(core::mem::size_of::<PollFd>() == 8);
const _: () = assert!(core::mem::offset_of!(PollFd, fd) == 0);
const _: () = assert!(core::mem::offset_of!(PollFd, events) == 4);
const _: () = assert!(core::mem::offset_of!(PollFd, revents) == 6);

/// Read up to `buf.len()` bytes from `fd` into `buf`. Returns the byte count
/// (0 at end-of-file).
pub fn read(fd: i32, buf: &mut [u8]) -> Result<usize, Errno> {
    // SAFETY: `buf` is a valid, exclusively-borrowed slice of `buf.len()`
    // bytes; the kernel writes at most that many.
    let ret = unsafe { syscall3(nr::READ, fd as usize, buf.as_mut_ptr() as usize, buf.len()) };
    from_ret(ret)
}

/// Wait for events on `fds`, up to `timeout` milliseconds (negative blocks
/// indefinitely). Returns the number of fds with non-zero `revents`.
pub fn poll(fds: &mut [PollFd], timeout: i32) -> Result<usize, Errno> {
    // SAFETY: `fds` is a valid, exclusively-borrowed slice of `fds.len()`
    // `PollFd` entries; the kernel only writes each `revents` field.
    let ret = unsafe {
        syscall3(
            nr::POLL,
            fds.as_mut_ptr() as usize,
            fds.len(),
            timeout as usize,
        )
    };
    from_ret(ret)
}

/// Create a pipe, returning `(read_end, write_end)`. `flags` accepts e.g.
/// `O_CLOEXEC`/`O_NONBLOCK` (raw values; callers supply them).
pub fn pipe2(flags: i32) -> Result<(i32, i32), Errno> {
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid array of two i32s; the kernel fills both.
    let ret = unsafe { syscall2(nr::PIPE2, fds.as_mut_ptr() as usize, flags as usize) };
    from_ret(ret)?;
    Ok((fds[0], fds[1]))
}

/// Duplicate `fd`, returning the lowest-numbered free descriptor.
pub fn dup(fd: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::DUP, fd as usize) };
    from_ret_i32(ret)
}

/// Duplicate `oldfd` onto `newfd`, closing `newfd` first if open. Returns
/// `newfd`.
pub fn dup2(oldfd: i32, newfd: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall2(nr::DUP2, oldfd as usize, newfd as usize) };
    from_ret_i32(ret)
}

/// Close `fd`.
pub fn close(fd: i32) -> Result<(), Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::CLOSE, fd as usize) };
    from_ret(ret).map(|_| ())
}

/// Perform an `fcntl(2)` operation with an integer argument (covers the
/// `F_GETFD`/`F_SETFD`/`FD_CLOEXEC` set rush needs).
pub fn fcntl(fd: i32, cmd: i32, arg: i32) -> Result<i32, Errno> {
    // SAFETY: integer command and argument; no pointer is dereferenced for the
    // commands exposed here.
    let ret = unsafe { syscall3(nr::FCNTL, fd as usize, cmd as usize, arg as usize) };
    from_ret_i32(ret)
}

/// Crate-internal `ioctl(2)` shim for the terminal queries.
///
/// # Safety
/// `arg` must be a valid pointer appropriate for `request` (e.g. `*mut
/// Termios` for `TCGETS`), or an integer request may ignore it.
pub(crate) unsafe fn ioctl(fd: i32, request: usize, arg: usize) -> Result<usize, Errno> {
    // SAFETY: forwarded to the caller's contract on `arg`/`request`.
    let ret = unsafe { syscall3(nr::IOCTL, fd as usize, request, arg) };
    from_ret(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_read_write_roundtrip() {
        let (r, w) = pipe2(0).expect("pipe2");
        // Write via a raw libc-free path: reuse our own read on the other end
        // after pushing bytes with the write syscall through std is awkward, so
        // exercise poll + close semantics here and bytes below.
        // Use std's File to write into the pipe's write end.
        use std::io::Write;
        use std::os::fd::FromRawFd;
        let mut wf = unsafe { std::fs::File::from_raw_fd(w) };
        wf.write_all(b"hello").unwrap();
        drop(wf); // closes w

        let mut buf = [0u8; 16];
        let n = read(r, &mut buf).expect("read");
        assert_eq!(&buf[..n], b"hello");
        close(r).expect("close r");
    }

    #[test]
    fn poll_reports_readable() {
        let (r, w) = pipe2(0).expect("pipe2");
        use std::io::Write;
        use std::os::fd::FromRawFd;
        let mut wf = unsafe { std::fs::File::from_raw_fd(w) };
        wf.write_all(b"x").unwrap();

        let mut fds = [PollFd {
            fd: r,
            events: POLLIN,
            revents: 0,
        }];
        let n = poll(&mut fds, 1000).expect("poll");
        assert_eq!(n, 1);
        assert!(fds[0].revents & POLLIN != 0);

        drop(wf);
        close(r).expect("close r");
    }

    #[test]
    fn dup_and_fcntl_cloexec() {
        let (r, w) = pipe2(0).expect("pipe2");
        let d = dup(r).expect("dup");
        assert!(d >= 0 && d != r);

        // Round-trip the CLOEXEC flag through fcntl.
        let flags = fcntl(d, F_GETFD, 0).expect("F_GETFD");
        assert_eq!(flags & FD_CLOEXEC, 0);
        fcntl(d, F_SETFD, FD_CLOEXEC).expect("F_SETFD");
        let flags = fcntl(d, F_GETFD, 0).expect("F_GETFD");
        assert_eq!(flags & FD_CLOEXEC, FD_CLOEXEC);

        for fd in [r, w, d] {
            close(fd).expect("close");
        }
    }

    #[test]
    fn close_bad_fd_is_ebadf() {
        assert_eq!(close(-1), Err(Errno(9))); // EBADF
    }
}
