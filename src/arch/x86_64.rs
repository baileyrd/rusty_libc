//! x86_64 Linux syscall stubs and syscall numbers.
//!
//! Calling convention: syscall number in `rax`; args in `rdi`, `rsi`, `rdx`,
//! `r10`, `r8`, `r9`; result in `rax`. The `syscall` instruction clobbers
//! `rcx` and `r11` and restores `rflags` from `r11`, so condition flags are
//! preserved (`preserves_flags` is sound). We never claim `nomem`: many
//! syscalls read or write caller memory through pointer arguments.

use core::arch::asm;

/// x86_64 syscall numbers used by this crate (from `<asm/unistd_64.h>`).
#[allow(missing_docs)] // each constant's name is its documentation.
pub mod nr {
    pub const READ: usize = 0;
    pub const WRITE: usize = 1;
    pub const OPEN: usize = 2;
    pub const CLOSE: usize = 3;
    pub const POLL: usize = 7;
    pub const LSEEK: usize = 8;
    pub const RT_SIGACTION: usize = 13;
    pub const RT_SIGPROCMASK: usize = 14;
    pub const RT_SIGRETURN: usize = 15;
    pub const IOCTL: usize = 16;
    pub const DUP: usize = 32;
    pub const DUP2: usize = 33;
    pub const GETPID: usize = 39;
    pub const CLONE: usize = 56;
    pub const EXECVE: usize = 59;
    pub const KILL: usize = 62;
    pub const MEMFD_CREATE: usize = 319;
    pub const WAIT4: usize = 61;
    pub const FCNTL: usize = 72;
    pub const GETCWD: usize = 79;
    pub const CHDIR: usize = 80;
    pub const FCHDIR: usize = 81;
    pub const UMASK: usize = 95;
    pub const GETUID: usize = 102;
    pub const SETPGID: usize = 109;
    pub const GETPPID: usize = 110;
    pub const SETSID: usize = 112;
    pub const GETPGID: usize = 121;
    pub const GETSID: usize = 124;
    pub const OPENAT: usize = 257;
    pub const EXIT_GROUP: usize = 231;
    pub const PIPE2: usize = 293;
    pub const PRLIMIT64: usize = 302;
    pub const EXECVEAT: usize = 322;
}

/// Issue syscall `n` with no arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall0(n: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 1 argument; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall1(n: usize, a1: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 2 arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall2(n: usize, a1: usize, a2: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 3 arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 4 arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall4(n: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 5 arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall5(n: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Issue syscall `n` with 6 arguments; returns the raw `rax` result.
#[inline]
pub unsafe fn syscall6(
    n: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}
