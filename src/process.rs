//! Process identity, process groups, signalling, and exit.
//!
//! `fork` is intentionally absent: per `DESIGN.md`, rush's forked children keep
//! running Rust on an inherited glibc heap, so `fork` stays on glibc until a
//! thread-quiescence plan exists (Phase 4).

use crate::arch::nr;
use crate::arch::{from_ret, syscall0, syscall1, syscall2, Errno};

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
}
