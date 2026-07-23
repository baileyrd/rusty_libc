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
    pub const PREAD64: usize = 17;
    pub const PWRITE64: usize = 18;
    pub const READV: usize = 19;
    pub const WRITEV: usize = 20;
    pub const FTRUNCATE: usize = 77;
    pub const NANOSLEEP: usize = 35;
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
    pub const GETGID: usize = 104;
    pub const GETEUID: usize = 107;
    pub const GETEGID: usize = 108;
    pub const GETGROUPS: usize = 115;
    pub const GETPRIORITY: usize = 140;
    pub const SETPRIORITY: usize = 141;
    pub const PRCTL: usize = 157;
    pub const UNAME: usize = 63;
    pub const SETPGID: usize = 109;
    pub const GETPPID: usize = 110;
    pub const SETSID: usize = 112;
    pub const GETPGID: usize = 121;
    pub const WAITID: usize = 247;
    pub const GETSID: usize = 124;
    pub const RT_SIGPENDING: usize = 127;
    pub const RT_SIGSUSPEND: usize = 130;
    pub const SIGNALFD4: usize = 289;
    pub const CLOCK_GETTIME: usize = 228;
    pub const TIMERFD_CREATE: usize = 283;
    pub const TIMERFD_SETTIME: usize = 286;
    pub const TIMERFD_GETTIME: usize = 287;
    pub const EXIT_GROUP: usize = 231;
    pub const OPENAT: usize = 257;
    pub const MKDIRAT: usize = 258;
    pub const UNLINKAT: usize = 263;
    pub const SYMLINKAT: usize = 266;
    pub const LINKAT: usize = 265;
    pub const READLINKAT: usize = 267;
    pub const FACCESSAT: usize = 269;
    pub const FCHOWNAT: usize = 260;
    pub const FCHMODAT: usize = 268;
    pub const UTIMENSAT: usize = 280;
    pub const DUP3: usize = 292;
    pub const PIPE2: usize = 293;
    pub const PRLIMIT64: usize = 302;
    pub const RENAMEAT2: usize = 316;
    pub const EXECVEAT: usize = 322;
    pub const STATX: usize = 332;
    pub const GETDENTS64: usize = 217;
    pub const PIDFD_OPEN: usize = 434;
    pub const CLONE3: usize = 435;
    pub const PIDFD_SEND_SIGNAL: usize = 424;
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

/// `clone(2)` with `CLONE_VFORK|CLONE_VM` (`flags`), immediately followed
/// in the child by `execve(2)` on `path`/`argv`/`envp` — entirely within
/// this one asm block, so the child executes not one instruction of
/// compiler-generated Rust/C code of its own.
///
/// This is deliberate, and not just a style choice: a `clone(CLONE_VM)`
/// child shares actual memory with the parent (no copy-on-write, unlike
/// plain `fork`), so any ordinary Rust code running in the child after the
/// syscall returns is unsound in two ways a bare `test`/`jnz`/`syscall`
/// sequence avoids. First, the child would need to *return* through the
/// same call frames the parent's own continuation resumes through — frames
/// that live on whatever stack address `clone` used, which for a freshly
/// provided child stack has none of the call/return bookkeeping already
/// resuming code depends on, and for a shared stack (`stack == 0`, as used
/// here) is fine for returning but reintroduces the second problem: the
/// compiler freely reuses a stack slot between "this local is only live in
/// the child branch" and "this local is only live in the parent's
/// continuation" — a correct optimization for code that, as far as the
/// compiler can see, executes at most one of those branches per call, but
/// wrong here because `CLONE_VM` really does let both branches' writes
/// land in the same physical memory, at genuinely different times. Doing
/// only raw syscalls, entirely in registers, sidesteps both: nothing is
/// ever pushed onto the (unused, since `stack == 0`) child stack, and
/// there is no Rust-level local for the optimizer to alias.
///
/// On `execve` failure the child calls `exit_group(127)`, matching the
/// shell "command not found" convention — the caller distinguishes "ran
/// and exited 127 itself" from "exec failed" the same way any fork+exec
/// caller already must, there being no channel back to the parent that
/// wouldn't reintroduce the hazard above.
///
/// Returns the child's pid to the parent, or the raw `-errno` `clone`
/// itself failed with (same `-4095..=-1` convention as every other raw
/// syscall in this crate). Never returns in the child.
///
/// # Safety
/// `path`, `argv`, `envp` must satisfy the same contract as `execve`:
/// `path` a valid, NUL-terminated C string, `argv`/`envp` valid
/// NUL-terminated arrays of valid C-string pointers, all live for the
/// duration of the call.
#[inline]
pub unsafe fn vfork_execve(
    flags: usize,
    path: *const u8,
    argv: *const *const u8,
    envp: *const *const u8,
) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",              // clone(flags, stack = 0, 0, 0, 0)
            "test rax, rax",
            "jnz 2f",               // parent: rax already holds the result
            "mov rdi, r12",
            "mov rsi, r13",
            "mov rdx, r14",
            "mov rax, {execve_nr}",
            "syscall",              // execve(path, argv, envp)
            "mov rax, {exit_nr}",
            "mov rdi, 127",
            "syscall",              // exit_group(127): execve failed
            "ud2",                  // unreachable: exit_group never returns
            "2:",
            inlateout("rax") nr::CLONE => ret,
            in("rdi") flags,
            in("rsi") 0usize,
            in("rdx") 0usize,
            in("r10") 0usize,
            in("r8") 0usize,
            in("r12") path,
            in("r13") argv,
            in("r14") envp,
            execve_nr = const nr::EXECVE,
            exit_nr = const nr::EXIT_GROUP,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    ret
}
