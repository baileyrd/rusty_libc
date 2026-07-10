//! Process identity, process groups, signalling, and exit, plus the raw
//! [`fork`] primitive (Phase 4).

use crate::arch::nr;
use crate::arch::{from_ret, from_ret_i32, syscall0, syscall1, syscall2, syscall5, Errno};

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
pub fn killpg(pgrp: i32, sig: i32) -> Result<(), Errno> {
    kill(pgrp.wrapping_neg(), sig)
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
    fn setpgid_self_is_noop_ok() {
        // Making the process its own group leader (or re-affirming it) either
        // succeeds or fails with EPERM depending on session state; both are
        // valid, non-panicking outcomes. Assert it does not blow up.
        let _ = setpgid(0, 0);
    }

    #[test]
    fn fork_child_runs_and_is_reaped() {
        use crate::fd;
        use crate::wait;

        // The child talks to the parent over a pipe, then exits. It runs in a
        // multithreaded test harness, so it must stay strictly async-signal-
        // safe: only raw syscalls, no allocation (that is the very hazard
        // `fork`'s safety note describes). `exit_group` ends it without running
        // any destructors.
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
}
