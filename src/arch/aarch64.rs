//! aarch64 Linux syscall stubs and syscall numbers.
//!
//! Calling convention: syscall number in `x8`; args in `x0`..`x5`; result in
//! `x0`. The `svc #0` instruction traps to the kernel, which preserves every
//! register except `x0` (and the condition flags), so no clobbers beyond the
//! `x0` result are required and `preserves_flags` is sound. As on x86_64 we
//! never claim `nomem`: syscalls read/write caller memory through pointers.
//!
//! Numbers come from the generic syscall table (`<asm-generic/unistd.h>`) that
//! aarch64 uses; several differ from x86_64's, and aarch64 lacks `poll`,
//! `dup2`, and the legacy `select`/`open` entirely (callers use `ppoll` and
//! `dup3`). No `rt_sigreturn` restorer is needed: the kernel points signal
//! returns at its vDSO `__kernel_rt_sigreturn`.

use core::arch::asm;

/// aarch64 syscall numbers used by this crate (from `<asm-generic/unistd.h>`).
pub mod nr {
    pub const DUP: usize = 23;
    pub const DUP3: usize = 24;
    pub const FCNTL: usize = 25;
    pub const IOCTL: usize = 29;
    pub const CLOSE: usize = 57;
    pub const PIPE2: usize = 59;
    pub const READ: usize = 63;
    pub const PPOLL: usize = 73;
    pub const EXIT_GROUP: usize = 94;
    pub const KILL: usize = 129;
    pub const RT_SIGACTION: usize = 134;
    pub const SETPGID: usize = 154;
    pub const UMASK: usize = 166;
    pub const GETPID: usize = 172;
    pub const GETPPID: usize = 173;
    pub const GETUID: usize = 174;
    pub const WAIT4: usize = 260;
    pub const PRLIMIT64: usize = 261;
}

#[inline]
pub unsafe fn syscall0(n: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            out("x0") ret,
            options(nostack, preserves_flags),
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall1(n: usize, a1: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            options(nostack, preserves_flags),
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall2(n: usize, a1: usize, a2: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            options(nostack, preserves_flags),
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            options(nostack, preserves_flags),
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall4(n: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            in("x3") a4,
            options(nostack, preserves_flags),
        );
    }
    ret
}

#[inline]
pub unsafe fn syscall5(n: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            in("x3") a4,
            in("x4") a5,
            options(nostack, preserves_flags),
        );
    }
    ret
}

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
            "svc #0",
            in("x8") n,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            in("x3") a4,
            in("x4") a5,
            in("x5") a6,
            options(nostack, preserves_flags),
        );
    }
    ret
}
