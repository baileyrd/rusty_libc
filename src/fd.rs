//! File-descriptor primitives: `read`, `poll`, `pipe2`, `dup`/`dup2`, `close`,
//! `fcntl`, and the crate-internal `ioctl` shim shared by [`crate::termios`]
//! and [`crate::tty`].

use crate::arch::nr;
#[cfg(target_arch = "aarch64")]
use crate::arch::syscall5;
use crate::arch::{from_ret, from_ret_i32, syscall1, syscall2, syscall3, Errno};

/// `poll(2)` event/return flag: data available to read.
pub const POLLIN: i16 = 0x001;
/// `poll(2)` event/return flag: urgent/priority data available to read.
pub const POLLPRI: i16 = 0x002;
/// `poll(2)` event/return flag: writing will not block.
pub const POLLOUT: i16 = 0x004;
/// `poll(2)` return-only flag: an error condition occurred.
pub const POLLERR: i16 = 0x008;
/// `poll(2)` return-only flag: the peer hung up (e.g. the pipe's writer closed).
pub const POLLHUP: i16 = 0x010;
/// `poll(2)` return-only flag: the fd is not open / invalid.
pub const POLLNVAL: i16 = 0x020;

/// `fcntl(2)` command: get the file-descriptor flags.
pub const F_GETFD: i32 = 1;
/// `fcntl(2)` command: set the file-descriptor flags.
pub const F_SETFD: i32 = 2;
/// `fcntl(2)` command: get the file-status flags (the `O_*` open flags).
pub const F_GETFL: i32 = 3;
/// `fcntl(2)` command: set the file-status flags (e.g. toggle [`O_NONBLOCK`]).
pub const F_SETFL: i32 = 4;
/// `fcntl(2)` command: like `F_DUPFD` but sets close-on-exec on the new fd.
pub const F_DUPFD_CLOEXEC: i32 = 1030;
/// File-descriptor flag: close the fd on `execve`.
pub const FD_CLOEXEC: i32 = 1;

/// Open/`pipe2`/`fcntl` file-status flag: set close-on-exec atomically.
pub const O_CLOEXEC: i32 = 0o2000000;
/// Open/`pipe2`/`fcntl` file-status flag: non-blocking I/O.
pub const O_NONBLOCK: i32 = 0o0004000;

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

impl PollFd {
    /// Construct a request watching `fd` for `events` (an OR of `POLL*`
    /// flags), with `revents` cleared.
    #[inline]
    pub const fn new(fd: i32, events: i16) -> Self {
        PollFd {
            fd,
            events,
            revents: 0,
        }
    }

    /// True if the kernel reported [`POLLIN`] (data available to read).
    #[inline]
    pub const fn is_readable(self) -> bool {
        self.revents & POLLIN != 0
    }

    /// True if the kernel reported [`POLLOUT`] (writing will not block).
    #[inline]
    pub const fn is_writable(self) -> bool {
        self.revents & POLLOUT != 0
    }

    /// True if the kernel reported [`POLLHUP`] (the peer hung up).
    #[inline]
    pub const fn is_hup(self) -> bool {
        self.revents & POLLHUP != 0
    }

    /// True if the kernel reported [`POLLERR`] (an error condition).
    #[inline]
    pub const fn is_error(self) -> bool {
        self.revents & POLLERR != 0
    }

    /// True if the kernel reported [`POLLNVAL`] (the fd is invalid).
    #[inline]
    pub const fn is_invalid(self) -> bool {
        self.revents & POLLNVAL != 0
    }
}

/// Read up to `buf.len()` bytes from `fd` into `buf`. Returns the byte count
/// (0 at end-of-file).
pub fn read(fd: i32, buf: &mut [u8]) -> Result<usize, Errno> {
    // SAFETY: `buf` is a valid, exclusively-borrowed slice of `buf.len()`
    // bytes; the kernel writes at most that many.
    let ret = unsafe { syscall3(nr::READ, fd as usize, buf.as_mut_ptr() as usize, buf.len()) };
    from_ret(ret)
}

/// Write up to `buf.len()` bytes from `buf` to `fd`. Returns the byte count
/// actually written (may be short).
pub fn write(fd: i32, buf: &[u8]) -> Result<usize, Errno> {
    // SAFETY: `buf` is a valid slice of `buf.len()` bytes the kernel only reads.
    let ret = unsafe { syscall3(nr::WRITE, fd as usize, buf.as_ptr() as usize, buf.len()) };
    from_ret(ret)
}

/// Wait for events on `fds`, up to `timeout` milliseconds (negative blocks
/// indefinitely). Returns the number of fds with non-zero `revents`.
///
/// x86_64 uses `poll`; aarch64 has no `poll` syscall, so this issues `ppoll`
/// with an equivalent `timespec` and a null signal mask. The behaviour and
/// signature are identical across arches.
#[cfg(target_arch = "x86_64")]
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

/// aarch64 `ppoll`-backed [`poll`]; see the x86_64 variant for docs.
#[cfg(target_arch = "aarch64")]
pub fn poll(fds: &mut [PollFd], timeout: i32) -> Result<usize, Errno> {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    // Negative timeout → null timespec (block indefinitely), matching poll.
    let ts = if timeout < 0 {
        None
    } else {
        Some(Timespec {
            tv_sec: (timeout / 1000) as i64,
            tv_nsec: (timeout % 1000) as i64 * 1_000_000,
        })
    };
    let tsp = match &ts {
        Some(t) => t as *const Timespec as usize,
        None => 0,
    };
    // ppoll(fds, nfds, tmo, sigmask = NULL, sigsetsize = 8). The kernel
    // ignores sigsetsize when sigmask is null, but pass the canonical 8.
    // SAFETY: `fds` is valid and exclusively borrowed; `tsp` is either null or
    // a valid `*const timespec`; the signal mask is null.
    let ret = unsafe { syscall5(nr::PPOLL, fds.as_mut_ptr() as usize, fds.len(), tsp, 0, 8) };
    from_ret(ret)
}

/// Create a pipe, returning `(read_end, write_end)`. `flags` accepts an OR of
/// [`O_CLOEXEC`]/[`O_NONBLOCK`] (or `0` for none).
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
///
/// aarch64 has no `dup2`; this emulates it with `dup3`. The one behavioural
/// gap dup3 has is `oldfd == newfd`: dup2 returns `newfd` unchanged when
/// `oldfd` is valid, whereas dup3 rejects it with `EINVAL`. We special-case
/// that to preserve dup2 semantics (validating `oldfd` via `fcntl`).
#[cfg(target_arch = "x86_64")]
pub fn dup2(oldfd: i32, newfd: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall2(nr::DUP2, oldfd as usize, newfd as usize) };
    from_ret_i32(ret)
}

/// aarch64 `dup3`-backed [`dup2`]; see the x86_64 variant for docs.
#[cfg(target_arch = "aarch64")]
pub fn dup2(oldfd: i32, newfd: i32) -> Result<i32, Errno> {
    if oldfd == newfd {
        // dup2 returns newfd if oldfd is valid, else EBADF. fcntl(F_GETFD)
        // validates oldfd with exactly that error.
        fcntl(oldfd, F_GETFD, 0)?;
        return Ok(newfd);
    }
    // SAFETY: plain integer arguments; flags = 0.
    let ret = unsafe { syscall3(nr::DUP3, oldfd as usize, newfd as usize, 0) };
    from_ret_i32(ret)
}

/// Close `fd`.
pub fn close(fd: i32) -> Result<(), Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::CLOSE, fd as usize) };
    from_ret(ret).map(|_| ())
}

/// Perform an `fcntl(2)` operation with an integer argument. Covers the
/// descriptor-flag commands ([`F_GETFD`]/[`F_SETFD`] with [`FD_CLOEXEC`]), the
/// status-flag commands ([`F_GETFL`]/[`F_SETFL`] with [`O_NONBLOCK`]), and
/// [`F_DUPFD_CLOEXEC`].
pub fn fcntl(fd: i32, cmd: i32, arg: i32) -> Result<i32, Errno> {
    // SAFETY: integer command and argument; no pointer is dereferenced for the
    // commands exposed here.
    let ret = unsafe { syscall3(nr::FCNTL, fd as usize, cmd as usize, arg as usize) };
    from_ret_i32(ret)
}

/// `memfd_create(2)` flag: set close-on-exec on the returned descriptor.
pub const MFD_CLOEXEC: u32 = 0x0001;

/// `lseek(2)` whence: set the offset to `offset` bytes from the start.
pub const SEEK_SET: i32 = 0;
/// `lseek(2)` whence: set the offset relative to the current position.
pub const SEEK_CUR: i32 = 1;
/// `lseek(2)` whence: set the offset relative to end-of-file.
pub const SEEK_END: i32 = 2;

/// Create an anonymous, memory-backed file and return a descriptor for it.
///
/// The `name` (used only for `/proc/self/fd` and debugging) must be a
/// nul-terminated C string. This is the thread-free way to feed a here-document
/// into a child's input: write the body, `lseek` back to the start, and `dup2`
/// the descriptor onto the target fd.
pub fn memfd_create(name: &core::ffi::CStr, flags: u32) -> Result<i32, Errno> {
    // SAFETY: `name` is a valid nul-terminated C string; the kernel reads it.
    let ret = unsafe { syscall2(nr::MEMFD_CREATE, name.as_ptr() as usize, flags as usize) };
    from_ret_i32(ret)
}

/// Reposition the offset of `fd` per `whence` (a `SEEK_*` constant), returning
/// the resulting absolute offset.
pub fn lseek(fd: i32, offset: i64, whence: i32) -> Result<i64, Errno> {
    // On 64-bit Linux `lseek` takes a 64-bit `off_t` directly.
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall3(nr::LSEEK, fd as usize, offset as usize, whence as usize) };
    from_ret(ret).map(|v| v as i64)
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

        let mut fds = [PollFd::new(r, POLLIN)];
        let n = poll(&mut fds, 1000).expect("poll");
        assert_eq!(n, 1);
        assert!(fds[0].is_readable());

        drop(wf);
        close(r).expect("close r");
    }

    #[test]
    fn poll_reports_hup_when_writer_closes() {
        let (r, w) = pipe2(0).expect("pipe2");
        // Close the write end with no data pending: the read end reports HUP.
        close(w).expect("close w");

        let mut fds = [PollFd::new(r, POLLIN)];
        let n = poll(&mut fds, 1000).expect("poll");
        assert_eq!(n, 1);
        assert!(fds[0].is_hup());
        close(r).expect("close r");
    }

    #[test]
    fn fcntl_toggles_nonblock() {
        let (r, w) = pipe2(0).expect("pipe2");

        // Initially blocking: O_NONBLOCK clear in the status flags.
        let flags = fcntl(r, F_GETFL, 0).expect("F_GETFL");
        assert_eq!(flags & O_NONBLOCK, 0);

        // Set non-blocking, then a read on the empty pipe returns EAGAIN
        // instead of blocking.
        fcntl(r, F_SETFL, flags | O_NONBLOCK).expect("F_SETFL");
        assert_eq!(fcntl(r, F_GETFL, 0).unwrap() & O_NONBLOCK, O_NONBLOCK);
        let mut buf = [0u8; 1];
        assert_eq!(read(r, &mut buf), Err(Errno::EAGAIN));

        close(r).expect("close r");
        close(w).expect("close w");
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
        assert_eq!(close(-1), Err(Errno::EBADF));
    }

    #[test]
    fn memfd_write_seek_read_roundtrip() {
        // The thread-free here-doc path: create → write body → rewind → read.
        let fd = memfd_create(c"rusty_libc_test", MFD_CLOEXEC).expect("memfd_create");
        assert!(fd >= 0);

        assert_eq!(write(fd, b"here-doc body").expect("write"), 13);
        // After writing, the offset is at the end; rewind to read it back.
        assert_eq!(lseek(fd, 0, SEEK_SET).expect("lseek"), 0);

        let mut buf = [0u8; 32];
        let n = read(fd, &mut buf).expect("read");
        assert_eq!(&buf[..n], b"here-doc body");

        // It is a real seekable file, unlike a pipe: SEEK_END gives the size.
        assert_eq!(lseek(fd, 0, SEEK_END).expect("lseek end"), 13);

        close(fd).expect("close");
    }
}
