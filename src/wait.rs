//! `waitpid` (via `wait4`) and the `W*` status-decoding helpers.
//!
//! The status helpers are pure bit tests over the kernel's wait-status
//! encoding; they take the raw `i32` status filled in by [`waitpid`].

use crate::arch::nr;
use crate::arch::{from_ret_i32, syscall4, Errno};

/// `waitpid` option: return immediately if no child has changed state.
pub const WNOHANG: i32 = 1;
/// `waitpid` option: also report stopped children.
pub const WUNTRACED: i32 = 2;
/// `waitpid` option: also report continued children.
pub const WCONTINUED: i32 = 8;

/// Wait for a state change in a child process.
///
/// Returns `(pid, status)` where `pid` is the child that changed state (`0`
/// when [`WNOHANG`] is set and no child is ready) and `status` is the raw
/// value to pass to the `w*` helpers below.
pub fn waitpid(pid: i32, options: i32) -> Result<(i32, i32), Errno> {
    let mut status: i32 = 0;
    // wait4(pid, &mut status, options, rusage = NULL).
    // SAFETY: `status` is a valid `*mut i32`; `rusage` is null.
    let ret = unsafe {
        syscall4(
            nr::WAIT4,
            pid as usize,
            &mut status as *mut i32 as usize,
            options as usize,
            0,
        )
    };
    let child = from_ret_i32(ret)?;
    Ok((child, status))
}

/// True if the child terminated normally (via `exit`/`return`).
#[inline]
pub fn wifexited(status: i32) -> bool {
    (status & 0x7f) == 0
}

/// Exit code of a normally-terminated child (valid when [`wifexited`]).
#[inline]
pub fn wexitstatus(status: i32) -> i32 {
    (status >> 8) & 0xff
}

/// True if the child was terminated by a signal.
#[inline]
pub fn wifsignaled(status: i32) -> bool {
    // Neither "exited" (low 7 bits == 0) nor "stopped" (low 7 bits == 0x7f).
    let sig = status & 0x7f;
    sig != 0 && sig != 0x7f
}

/// Signal that terminated the child (valid when [`wifsignaled`]).
#[inline]
pub fn wtermsig(status: i32) -> i32 {
    status & 0x7f
}

/// True if the child is currently stopped.
#[inline]
pub fn wifstopped(status: i32) -> bool {
    (status & 0xff) == 0x7f
}

/// Signal that stopped the child (valid when [`wifstopped`]).
#[inline]
pub fn wstopsig(status: i32) -> i32 {
    (status >> 8) & 0xff
}

/// True if the child was resumed by `SIGCONT`.
#[inline]
pub fn wifcontinued(status: i32) -> bool {
    status == 0xffff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_normal_exit() {
        // Child exited with code 42: status == 42 << 8.
        let status = 42 << 8;
        assert!(wifexited(status));
        assert_eq!(wexitstatus(status), 42);
        assert!(!wifsignaled(status));
        assert!(!wifstopped(status));
    }

    #[test]
    fn decodes_signal_death() {
        // Killed by SIGKILL (9), no core dump: low 7 bits == 9.
        let status = 9;
        assert!(wifsignaled(status));
        assert_eq!(wtermsig(status), 9);
        assert!(!wifexited(status));
        assert!(!wifstopped(status));
    }

    #[test]
    fn decodes_stop_and_continue() {
        // Stopped by SIGSTOP (19): (19 << 8) | 0x7f.
        let status = (19 << 8) | 0x7f;
        assert!(wifstopped(status));
        assert_eq!(wstopsig(status), 19);
        assert!(!wifsignaled(status));

        assert!(wifcontinued(0xffff));
    }

    #[test]
    fn wnohang_no_child_is_einval_or_echild() {
        // No children exist: waitpid(-1, WNOHANG) fails with ECHILD (10).
        assert_eq!(waitpid(-1, WNOHANG), Err(Errno(10)));
    }
}
