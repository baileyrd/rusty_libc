//! Process identity, process groups, signalling, and exit, plus the raw
//! [`fork`] primitive (Phase 4).

use crate::arch::nr;
use crate::arch::{
    from_ret, from_ret_i32, syscall0, syscall1, syscall2, syscall3, syscall5, Errno,
};
use core::ffi::{c_char, CStr};

/// Get the calling process's ID. Cannot fail.
#[inline]
pub fn getpid() -> i32 {
    // SAFETY: getpid takes no arguments and never fails.
    unsafe { syscall0(nr::GETPID) as i32 }
}

/// Get the parent process's ID. Cannot fail.
#[inline]
pub fn getppid() -> i32 {
    // SAFETY: getppid takes no arguments and never fails.
    unsafe { syscall0(nr::GETPPID) as i32 }
}

/// Get the calling process's real user ID. Cannot fail.
#[inline]
pub fn getuid() -> u32 {
    // SAFETY: getuid takes no arguments and never fails.
    unsafe { syscall0(nr::GETUID) as u32 }
}

/// Get the calling process's effective user ID. Cannot fail.
///
/// This is the id the kernel checks for permissions; a shell compares it to `0`
/// to decide the root prompt (`#` vs `$`).
#[inline]
pub fn geteuid() -> u32 {
    // SAFETY: geteuid takes no arguments and never fails.
    unsafe { syscall0(nr::GETEUID) as u32 }
}

/// Get the calling process's real group ID. Cannot fail.
#[inline]
pub fn getgid() -> u32 {
    // SAFETY: getgid takes no arguments and never fails.
    unsafe { syscall0(nr::GETGID) as u32 }
}

/// Get the calling process's effective group ID. Cannot fail.
#[inline]
pub fn getegid() -> u32 {
    // SAFETY: getegid takes no arguments and never fails.
    unsafe { syscall0(nr::GETEGID) as u32 }
}

/// Set the process group ID of `pid` to `pgid` (both `0` mean "self").
pub fn setpgid(pid: i32, pgid: i32) -> Result<(), Errno> {
    // SAFETY: plain integer arguments, no memory referenced.
    let ret = unsafe { syscall2(nr::SETPGID, pid as usize, pgid as usize) };
    from_ret(ret).map(|_| ())
}

/// Send signal `sig` to `pid` (see `kill(2)` for the `pid` sign conventions).
pub fn kill(pid: i32, sig: i32) -> Result<(), Errno> {
    // SAFETY: plain integer arguments, no memory referenced.
    let ret = unsafe { syscall2(nr::KILL, pid as usize, sig as usize) };
    from_ret(ret).map(|_| ())
}

/// Send signal `sig` to every process in process group `pgrp`.
///
/// Equivalent to `kill(-pgrp, sig)`; `pgrp == 0` targets the caller's group.
#[inline]
pub fn killpg(pgrp: i32, sig: i32) -> Result<(), Errno> {
    // `kill` reads a negative pid as "process group -pid". Negating maps `pgrp`
    // to that form; `0.wrapping_neg() == 0`, and `kill(0, sig)` already means
    // "the caller's own group", so the `pgrp == 0` case falls out correctly.
    kill(pgrp.wrapping_neg(), sig)
}

/// Open a file descriptor referring to process `pid` (`pidfd_open(2)`,
/// Track P). Unlike a bare pid number, the returned fd stays a stable
/// handle to *this specific* process even if `pid` exits and the kernel
/// recycles the number for an unrelated process later — the reuse race a
/// raw pid can't avoid. Poll it for readability (readable once the
/// process exits) or wait on it via `waitid(P_PIDFD, ...)`.
///
/// `flags` must be `0` — the kernel defines no flags for this syscall as
/// of the versions this crate targets; a future one (e.g. `PIDFD_NONBLOCK`)
/// would be admitted here when a consumer needs it.
///
/// The returned fd is `O_CLOEXEC` by kernel default and must be closed
/// with [`crate::fd::close`] like any other fd.
pub fn pidfd_open(pid: i32, flags: u32) -> Result<i32, Errno> {
    // SAFETY: plain integer arguments, no memory referenced.
    let ret = unsafe { syscall2(nr::PIDFD_OPEN, pid as usize, flags as usize) };
    from_ret_i32(ret)
}

/// Terminate all threads in the process with status `status`. Never returns.
pub fn exit_group(status: i32) -> ! {
    // SAFETY: exit_group never returns; the kernel tears the process down.
    unsafe {
        syscall1(nr::EXIT_GROUP, status as usize);
        // Unreachable, but keep the type `!` honest if the kernel ever did.
        core::hint::unreachable_unchecked()
    }
}

/// `SIGCHLD`: sent to the parent on child termination. Passed to `clone` as the
/// low byte of the flags so a plain wait reaps the child, matching `fork`.
const SIGCHLD: usize = 17;

/// Create a child process, returning the child's pid to the parent and `0` to
/// the child. Backed by `clone(SIGCHLD, stack = NULL, …)` — a null stack gives
/// the child a copy-on-write clone of the parent's stack, i.e. `fork`
/// semantics.
///
/// # Safety
///
/// This is a **raw** fork. Unlike glibc's `fork()`, it does **not** reset
/// glibc's internal malloc/stdio locks in the child, run `pthread_atfork`
/// handlers, or otherwise make a multithreaded parent safe. If any *other*
/// thread in the parent holds a lock (e.g. the malloc arena) at the instant of
/// the call, the child inherits it locked and deadlocks the first time it needs
/// it — and a Rust child that keeps running (rather than going straight to
/// `exec`/[`exit_group`]) will need the allocator almost immediately.
///
/// Only call this when the process is effectively single-threaded at the fork
/// point (no other thread can be mid-allocation), or when the child touches
/// nothing but async-signal-safe syscalls before `exec`/[`exit_group`]. See
/// rush's `LIBC_DEPENDENCY_ANALYSIS.md` §4.2.
pub unsafe fn fork() -> Result<i32, Errno> {
    // clone(flags = SIGCHLD, stack = 0, parent_tid = 0, child_tid = 0, tls = 0).
    // Argument order is the same on x86_64 and aarch64.
    // SAFETY: all pointer arguments are null; a null stack requests fork-style
    // copy-on-write of the caller's stack.
    let ret = unsafe { syscall5(nr::CLONE, SIGCHLD, 0, 0, 0, 0) };
    from_ret_i32(ret)
}

/// Replace the current process image with the program at `path`.
///
/// `argv` and `envp` must each point to a **null-terminated** array of C-string
/// pointers (`argv[0]` is conventionally the program name; `envp` may be a lone
/// null for an empty environment). On success this never returns — the process
/// image is replaced. It returns **only on failure**, yielding the [`Errno`].
///
/// # Safety
/// `argv` and `envp` must be valid, null-terminated arrays of valid C-string
/// pointers that live for the duration of the call, and `path` a valid C
/// string. After a `fork` in a multithreaded parent, the pre-`exec` child must
/// stay async-signal-safe (build the arrays without allocating); see [`fork`].
pub unsafe fn execve(path: &CStr, argv: *const *const c_char, envp: *const *const c_char) -> Errno {
    // SAFETY: pointer validity/termination is the caller's contract; the kernel
    // reads the C string and the two pointer arrays.
    let ret = unsafe {
        syscall3(
            nr::EXECVE,
            path.as_ptr() as usize,
            argv as usize,
            envp as usize,
        )
    };
    // execve returns only on error; decode the -errno. The `Ok` arm is
    // unreachable in practice (a successful exec never returns here).
    match from_ret(ret) {
        Ok(_) => Errno(0),
        Err(e) => e,
    }
}

/// Like [`execve`] but resolves `path` relative to `dirfd` (or [`AT_FDCWD`])
/// and takes `flags` (e.g. `AT_EMPTY_PATH` to exec an already-open fd).
///
/// [`AT_FDCWD`]: crate::fd::AT_FDCWD
///
/// # Safety
/// Same contract as [`execve`] for `path`/`argv`/`envp`.
pub unsafe fn execveat(
    dirfd: i32,
    path: &CStr,
    argv: *const *const c_char,
    envp: *const *const c_char,
    flags: i32,
) -> Errno {
    // SAFETY: as `execve`, plus `dirfd`/`flags` are plain integers.
    let ret = unsafe {
        syscall5(
            nr::EXECVEAT,
            dirfd as usize,
            path.as_ptr() as usize,
            argv as usize,
            envp as usize,
            flags as usize,
        )
    };
    match from_ret(ret) {
        Ok(_) => Errno(0),
        Err(e) => e,
    }
}

/// A null-terminated array of C-string pointers, owned and kept alive
/// alongside it — the shape [`execve`]/[`execveat`] need for both `argv` and
/// `envp` (they are structurally identical: a NUL-terminated list of C
/// strings). Requires the opt-in `std` feature; the `no_std` core has no
/// allocator to build this with, so callers there still hand-roll the arrays
/// as documented on [`execve`].
///
/// ```rust,ignore
/// use rusty_libc::process::CStrArray;
///
/// let argv = CStrArray::new(["/bin/sh", "-c", "exit 0"]).expect("no interior NUL");
/// let envp = CStrArray::new(["PATH=/bin"]).expect("no interior NUL");
/// unsafe { rusty_libc::process::execve(c"/bin/sh", argv.as_ptr(), envp.as_ptr()) };
/// ```
// Gated on `any(feature = "std", test)`, not just `feature = "std"`, matching
// `Errno`'s `std::io::Error` interop impl: the built-in test harness always
// links `std` (see the crate's top-level `cfg_attr(not(any(test, feature =
// "std")), no_std)`), so this lets the type's own tests run under a plain
// `cargo test` instead of only being compile-checked by the separate "Build
// (std feature)" CI step.
#[cfg(any(feature = "std", test))]
pub struct CStrArray {
    // Kept alive so `ptrs` stays valid; never read directly after construction.
    _owned: std::vec::Vec<std::ffi::CString>,
    // `_owned`'s pointers, plus a trailing null -- this is what `as_ptr` hands
    // to `execve`/`execveat`.
    ptrs: std::vec::Vec<*const c_char>,
}

#[cfg(any(feature = "std", test))]
impl CStrArray {
    /// Build a null-terminated C-string array from owned strings (or byte
    /// slices), copying each into a fresh [`std::ffi::CString`]. Fails if any
    /// input contains an interior NUL byte (which cannot be represented as a
    /// C string).
    pub fn new<I, S>(strings: I) -> Result<Self, std::ffi::NulError>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::vec::Vec<u8>>,
    {
        let owned = strings
            .into_iter()
            .map(std::ffi::CString::new)
            .collect::<Result<std::vec::Vec<_>, _>>()?;
        let mut ptrs: std::vec::Vec<*const c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(core::ptr::null());
        Ok(CStrArray {
            _owned: owned,
            ptrs,
        })
    }

    /// The null-terminated pointer array, ready to pass as `execve`'s `argv`
    /// or `envp`. Valid for as long as `self` is alive.
    #[inline]
    pub fn as_ptr(&self) -> *const *const c_char {
        self.ptrs.as_ptr()
    }
}

/// Change the calling process's working directory to `path`.
pub fn chdir(path: &CStr) -> Result<(), Errno> {
    // SAFETY: `path` is a valid nul-terminated C string the kernel only reads.
    let ret = unsafe { syscall1(nr::CHDIR, path.as_ptr() as usize) };
    from_ret(ret).map(|_| ())
}

/// Change the calling process's working directory to the one referred to by the
/// open descriptor `fd`.
pub fn fchdir(fd: i32) -> Result<(), Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::FCHDIR, fd as usize) };
    from_ret(ret).map(|_| ())
}

/// Write the absolute path of the current working directory into `buf` and
/// return the path bytes (**without** the trailing NUL).
///
/// Fails with `ERANGE` if `buf` is too small to hold the path and its NUL.
pub fn getcwd(buf: &mut [u8]) -> Result<&[u8], Errno> {
    // The kernel's getcwd returns the length *including* the NUL terminator.
    // SAFETY: `buf` is a valid, exclusively-borrowed slice of `buf.len()`
    // bytes; the kernel writes at most that many.
    let ret = unsafe { syscall2(nr::GETCWD, buf.as_mut_ptr() as usize, buf.len()) };
    let len = from_ret(ret)?;
    // len is >= 1 (at minimum "/\0"); drop the trailing NUL byte.
    Ok(&buf[..len.saturating_sub(1)])
}

/// Create a new session and set the process group, making the caller the
/// session and group leader. Returns the new session ID. Fails with `EPERM` if
/// the caller is already a process-group leader.
pub fn setsid() -> Result<i32, Errno> {
    // SAFETY: takes no arguments.
    let ret = unsafe { syscall0(nr::SETSID) };
    from_ret_i32(ret)
}

/// Get the process-group ID of `pid` (`0` means the calling process).
pub fn getpgid(pid: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::GETPGID, pid as usize) };
    from_ret_i32(ret)
}

/// Get the session ID of `pid` (`0` means the calling process).
pub fn getsid(pid: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer argument.
    let ret = unsafe { syscall1(nr::GETSID, pid as usize) };
    from_ret_i32(ret)
}

/// Get the calling process's own process-group ID. Convenience for
/// [`getpgid`]`(0)` — the classic no-argument `getpgrp()`.
///
/// Implemented via `getpgid(0)` so it is identical on both arches (aarch64 has
/// no dedicated `getpgrp` syscall); querying your own group cannot meaningfully
/// fail.
#[inline]
pub fn getpgrp() -> Result<i32, Errno> {
    getpgid(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_consistent() {
        assert!(getpid() > 0);
        assert!(getppid() > 0);
        // getpid must match the value std reports.
        assert_eq!(getpid() as u32, std::process::id());
    }

    #[test]
    fn credential_getters_are_consistent() {
        // The test process is not setuid/setgid, so effective == real, and the
        // getters are stable across calls.
        assert_eq!(geteuid(), getuid());
        assert_eq!(getegid(), getgid());
        assert_eq!(getuid(), getuid());
    }

    #[test]
    fn setpgid_self_is_noop_ok() {
        // Making the process its own group leader (or re-affirming it) either
        // succeeds or fails with EPERM depending on session state; both are
        // valid, non-panicking outcomes. Assert it does not blow up.
        let _ = setpgid(0, 0);
    }

    #[test]
    fn session_and_group_ids_are_positive() {
        // 0 means "the calling process" for both queries.
        let pgid = getpgid(0).expect("getpgid(0)");
        let sid = getsid(0).expect("getsid(0)");
        assert!(pgid > 0);
        assert!(sid > 0);
        // Querying by explicit pid agrees with the pid==0 shorthand.
        assert_eq!(getpgid(getpid()).expect("getpgid(pid)"), pgid);
        assert_eq!(getsid(getpid()).expect("getsid(pid)"), sid);
        // getpgrp() is getpgid(0).
        assert_eq!(getpgrp().expect("getpgrp"), pgid);
    }

    #[test]
    fn getpgid_missing_process_is_esrch() {
        // No process has this pid: getpgid reports ESRCH (3).
        assert_eq!(getpgid(0x3fff_ffff), Err(Errno(3)));
    }

    // chdir mutates process-global state, so all cwd assertions live in one
    // sequential test to avoid races with other tests (which all use absolute
    // paths and are unaffected as long as we restore the cwd promptly).
    #[test]
    fn chdir_fchdir_getcwd_roundtrip() {
        let mut buf = [0u8; 4096];
        let saved = getcwd(&mut buf).expect("getcwd").to_vec();

        // chdir to a known absolute directory and read it back.
        chdir(c"/").expect("chdir /");
        let mut buf2 = [0u8; 4096];
        assert_eq!(getcwd(&mut buf2).expect("getcwd /"), b"/");

        // fchdir to /tmp via an open directory fd, then confirm.
        let dirfd = crate::fd::open(c"/tmp", crate::fd::O_RDONLY | crate::fd::O_DIRECTORY, 0)
            .expect("open /tmp");
        fchdir(dirfd).expect("fchdir /tmp");
        let mut buf3 = [0u8; 4096];
        assert_eq!(getcwd(&mut buf3).expect("getcwd /tmp"), b"/tmp");
        crate::fd::close(dirfd).expect("close");

        // A too-small buffer yields ERANGE.
        let mut tiny = [0u8; 1];
        assert_eq!(getcwd(&mut tiny), Err(Errno::ERANGE));

        // Restore the original working directory.
        let back = std::ffi::CString::new(saved).unwrap();
        chdir(&back).expect("chdir back");
    }

    #[test]
    fn setsid_in_child_creates_new_session() {
        use crate::wait;
        // setsid fails with EPERM for a process-group leader, so exercise it in
        // a fresh child (which is not a group leader): it succeeds and returns
        // the new session id, equal to the child's own pid. The child stays
        // async-signal-safe (raw syscalls only); see `fork`'s safety note.
        match unsafe { fork() }.expect("fork") {
            0 => {
                let ok = matches!((setsid(), getpid()), (Ok(sid), pid) if sid == pid);
                exit_group(if ok { 0 } else { 1 });
            }
            pid => {
                let (_, status) = wait::waitpid(pid, 0).expect("waitpid");
                assert!(wait::wifexited(status));
                assert_eq!(wait::wexitstatus(status), 0);
            }
        }
    }

    #[test]
    fn execve_missing_file_returns_enoent() {
        // execve returns only on failure; a bad path never replaces our image,
        // so this is safe to run without a fork.
        let path = c"/nonexistent/rusty_libc/prog";
        let argv: [*const c_char; 2] = [path.as_ptr(), core::ptr::null()];
        let envp: [*const c_char; 1] = [core::ptr::null()];
        let e = unsafe { execve(path, argv.as_ptr(), envp.as_ptr()) };
        assert_eq!(e, Errno::ENOENT);
    }

    #[test]
    fn execveat_missing_file_returns_enoent() {
        // Regression test: `execveat`'s own syscall number, not `execve`'s.
        // aarch64's EXECVEAT constant was wrong for a while (387, an
        // unallocated number, instead of the real generic-ABI 281) and no
        // test called `execveat` directly, so it went uncaught -- `execve`'s
        // test above exercises a different `nr` constant entirely. This
        // fails with ENOSYS instead of ENOENT if the syscall number regresses.
        let path = c"/nonexistent/rusty_libc/prog";
        let argv: [*const c_char; 2] = [path.as_ptr(), core::ptr::null()];
        let envp: [*const c_char; 1] = [core::ptr::null()];
        let e = unsafe { execveat(crate::fd::AT_FDCWD, path, argv.as_ptr(), envp.as_ptr(), 0) };
        assert_eq!(e, Errno::ENOENT);
    }

    // The success path replaces the child image with a real binary. Gate it to
    // x86_64 so it runs natively; under the aarch64 qemu-user CI job a nested
    // execve of a host binary is not reliably emulated.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn execve_replaces_child_image() {
        use crate::wait;
        match unsafe { fork() }.expect("fork") {
            0 => {
                // exec `/bin/sh -c "exit 7"`; a delivered argv proves through
                // the child's exit code.
                let path = c"/bin/sh";
                let argv: [*const c_char; 4] = [
                    c"/bin/sh".as_ptr(),
                    c"-c".as_ptr(),
                    c"exit 7".as_ptr(),
                    core::ptr::null(),
                ];
                let envp: [*const c_char; 1] = [core::ptr::null()];
                unsafe { execve(path, argv.as_ptr(), envp.as_ptr()) };
                // Only reached if exec failed.
                exit_group(127);
            }
            pid => {
                let (_, status) = wait::waitpid(pid, 0).expect("waitpid");
                assert!(wait::wifexited(status));
                assert_eq!(wait::wexitstatus(status), 7);
            }
        }
    }

    #[test]
    fn cstrarray_builds_a_null_terminated_pointer_array() {
        let argv = CStrArray::new(["/bin/sh", "-c", "exit 0"]).expect("no interior NUL");
        // SAFETY: reading back what `new` just wrote, purely to check shape.
        let ptrs = unsafe { core::slice::from_raw_parts(argv.as_ptr(), 4) };
        assert!(!ptrs[0].is_null());
        assert!(!ptrs[1].is_null());
        assert!(!ptrs[2].is_null());
        assert!(ptrs[3].is_null(), "must be null-terminated");

        let s0 = unsafe { CStr::from_ptr(ptrs[0]) };
        assert_eq!(s0.to_bytes(), b"/bin/sh");
    }

    #[test]
    fn cstrarray_rejects_interior_nul() {
        assert!(CStrArray::new(["bad\0arg"]).is_err());
    }

    // Gated to x86_64 for the same reason as `execve_replaces_child_image`:
    // a nested execve of a host binary isn't reliably emulated under the
    // aarch64 qemu-user CI job.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn cstrarray_is_usable_end_to_end_with_execve() {
        use crate::wait;

        let argv = CStrArray::new(["/bin/sh", "-c", "exit 9"]).expect("no interior NUL");
        let envp = CStrArray::new::<_, &str>([]).expect("no interior NUL");

        match unsafe { fork() }.expect("fork") {
            0 => {
                unsafe { execve(c"/bin/sh", argv.as_ptr(), envp.as_ptr()) };
                exit_group(127); // only reached if exec failed
            }
            pid => {
                let (_, status) = wait::waitpid(pid, 0).expect("waitpid");
                assert!(wait::wifexited(status));
                assert_eq!(wait::wexitstatus(status), 9);
            }
        }
    }

    #[test]
    fn fork_child_runs_and_is_reaped() {
        use crate::fd;
        use crate::wait;

        // The child talks to the parent over a pipe, then exits. CI runs the
        // suite with `--test-threads=1` (see ci.yml) so the process is
        // effectively single-threaded at the fork point; even so, the child
        // stays strictly async-signal-safe as defense-in-depth: only raw
        // syscalls, no allocation (the very hazard `fork`'s safety note
        // describes). `exit_group` ends it without running any destructors.
        let (r, w) = fd::pipe2(0).expect("pipe2");
        match unsafe { fork() }.expect("fork") {
            0 => {
                let _ = fd::write(w, b"K");
                exit_group(7);
            }
            pid => {
                fd::close(w).expect("close w");
                let mut buf = [0u8; 1];
                let n = fd::read(r, &mut buf).expect("read");
                assert_eq!(&buf[..n], b"K");
                fd::close(r).expect("close r");

                let (wpid, status) = wait::waitpid(pid, 0).expect("waitpid");
                assert_eq!(wpid, pid);
                assert!(wait::wifexited(status));
                assert_eq!(wait::wexitstatus(status), 7);
            }
        }
    }

    #[test]
    fn pidfd_open_self_yields_a_pollable_fd() {
        use crate::fd;

        let pidfd = pidfd_open(getpid(), 0).expect("pidfd_open(self)");
        assert!(pidfd >= 0);
        // Not readable while the process is alive: a poll with a zero
        // timeout must time out (0 fds ready), not report POLLIN.
        let mut fds = [fd::PollFd {
            fd: pidfd,
            events: fd::POLLIN,
            revents: 0,
        }];
        let n = fd::poll(&mut fds, 0).expect("poll");
        assert_eq!(n, 0);
        fd::close(pidfd).expect("close pidfd");
    }

    #[test]
    fn pidfd_open_refuses_a_dead_pid() {
        // Fork a child, wait for it to exit, then confirm the kernel
        // refuses to open a pidfd for the now-gone pid — ESRCH, not a
        // silent success on a recycled/nonexistent identity.
        use crate::wait;

        let child = match unsafe { fork() }.expect("fork") {
            0 => exit_group(0),
            pid => pid,
        };
        wait::waitpid(child, 0).expect("waitpid");
        assert_eq!(pidfd_open(child, 0), Err(Errno::ESRCH));
    }
}
