//! Signal handling via `rt_sigaction`, plus the x86_64 `SA_RESTORER`
//! trampoline.
//!
//! This mirrors glibc's BSD-style `signal(3)`: the handler is persistent
//! (not reset after delivery) and installed with `SA_RESTART`, so slow
//! syscalls resume rather than failing with `EINTR`. The delivered signal is
//! blocked for the duration of its own handler by the kernel default.
//!
//! ## The x86_64 restorer problem
//!
//! On x86_64 the kernel has **no** default signal-return path: when a handler
//! returns, control resumes at whatever address `sa_restorer` points to, and
//! that code must invoke `SYS_rt_sigreturn` to unwind the kernel-pushed signal
//! frame. glibc supplies this trampoline; since we do not link glibc's
//! `sigaction`, we must supply our own. [`sigreturn_trampoline`] is a naked
//! function doing exactly `mov rax, RT_SIGRETURN; syscall`, and every
//! [`signal`] install sets `SA_RESTORER` and points `sa_restorer` at it.
//!
//! aarch64 (Phase 5) needs no restorer: its kernel installs a default vDSO
//! trampoline, so `SA_RESTORER` is neither set nor required there.

use crate::arch::nr;
use crate::arch::{from_ret, syscall4, Errno};

/// A signal handler: [`SIG_DFL`], [`SIG_IGN`], or a function pointer
/// (`extern "C" fn(i32)` cast to `usize`). Modelled as `usize` to match
/// libc's `sighandler_t`, which must also represent the two sentinel values.
pub type Sighandler = usize;

/// Default disposition for the signal.
pub const SIG_DFL: Sighandler = 0;
/// Ignore the signal.
pub const SIG_IGN: Sighandler = 1;

// Standard Linux signal numbers (x86_64 / asm-generic).
/// Hangup.
pub const SIGHUP: i32 = 1;
/// Interrupt (typically Ctrl-C).
pub const SIGINT: i32 = 2;
/// Quit (typically Ctrl-\).
pub const SIGQUIT: i32 = 3;
/// Illegal instruction.
pub const SIGILL: i32 = 4;
/// Trace/breakpoint trap.
pub const SIGTRAP: i32 = 5;
/// Abort.
pub const SIGABRT: i32 = 6;
/// Bus error.
pub const SIGBUS: i32 = 7;
/// Floating-point exception.
pub const SIGFPE: i32 = 8;
/// Kill (cannot be caught or ignored).
pub const SIGKILL: i32 = 9;
/// User-defined signal 1.
pub const SIGUSR1: i32 = 10;
/// Invalid memory reference.
pub const SIGSEGV: i32 = 11;
/// User-defined signal 2.
pub const SIGUSR2: i32 = 12;
/// Write to a pipe with no readers.
pub const SIGPIPE: i32 = 13;
/// Alarm clock.
pub const SIGALRM: i32 = 14;
/// Termination request.
pub const SIGTERM: i32 = 15;
/// Stack fault (unused on most systems).
pub const SIGSTKFLT: i32 = 16;
/// Child stopped or terminated.
pub const SIGCHLD: i32 = 17;
/// Continue if stopped.
pub const SIGCONT: i32 = 18;
/// Stop (cannot be caught or ignored).
pub const SIGSTOP: i32 = 19;
/// Terminal stop (typically Ctrl-Z).
pub const SIGTSTP: i32 = 20;
/// Background read from terminal.
pub const SIGTTIN: i32 = 21;
/// Background write to terminal.
pub const SIGTTOU: i32 = 22;
/// Urgent condition on socket.
pub const SIGURG: i32 = 23;
/// CPU time limit exceeded.
pub const SIGXCPU: i32 = 24;
/// File size limit exceeded.
pub const SIGXFSZ: i32 = 25;
/// Virtual alarm clock.
pub const SIGVTALRM: i32 = 26;
/// Profiling timer expired.
pub const SIGPROF: i32 = 27;
/// Window resize.
pub const SIGWINCH: i32 = 28;
/// I/O now possible.
pub const SIGIO: i32 = 29;
/// Power failure.
pub const SIGPWR: i32 = 30;
/// Bad system call.
pub const SIGSYS: i32 = 31;

/// `sa_flags`: resume slow syscalls instead of failing with `EINTR`.
const SA_RESTART: u64 = 0x1000_0000;
/// `sa_flags`: `sa_restorer` is valid and should be used (x86_64 requires it;
/// aarch64 has no restorer and never sets this).
#[cfg(target_arch = "x86_64")]
const SA_RESTORER: u64 = 0x0400_0000;

/// Size in bytes of the kernel `sigset_t` we pass to `rt_sigaction`. The
/// kernel checks this against its own expectation; on 64-bit Linux it is 8.
const SIGSETSIZE: usize = 8;

/// The kernel's `struct sigaction` (a.k.a. `struct kernel_sigaction`) for
/// x86_64. Note the field order is the **kernel's** — handler, flags,
/// restorer, mask — which differs from glibc's userspace layout.
#[repr(C)]
struct KernelSigaction {
    sa_handler: usize,
    sa_flags: u64,
    sa_restorer: usize,
    /// Signals blocked during the handler; a single 64-bit word here.
    sa_mask: u64,
}

const _: () = assert!(core::mem::size_of::<KernelSigaction>() == 32);
const _: () = assert!(core::mem::offset_of!(KernelSigaction, sa_handler) == 0);
const _: () = assert!(core::mem::offset_of!(KernelSigaction, sa_flags) == 8);
const _: () = assert!(core::mem::offset_of!(KernelSigaction, sa_restorer) == 16);
const _: () = assert!(core::mem::offset_of!(KernelSigaction, sa_mask) == 24);

/// The x86_64 signal-return trampoline: `sa_restorer` points here so that,
/// when a handler returns, this code issues `SYS_rt_sigreturn` to restore the
/// pre-signal context. Naked and never inlined so its address is a stable,
/// valid entry point. aarch64 needs no equivalent (kernel vDSO handles it).
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
unsafe extern "C" fn sigreturn_trampoline() {
    core::arch::naked_asm!(
        "mov rax, {nr}",
        "syscall",
        nr = const nr::RT_SIGRETURN,
    )
}

/// Install `handler` for signal `sig`, returning the previous handler.
///
/// The handler is persistent and installed with `SA_RESTART` (glibc BSD
/// `signal(3)` semantics). `handler` is [`SIG_DFL`], [`SIG_IGN`], or an
/// `extern "C" fn(i32)` cast to `usize`.
///
/// # Safety
/// Installing an arbitrary handler is inherently unsafe: the handler runs
/// asynchronously in signal context, where only async-signal-safe operations
/// are permitted, and `handler` (when not a sentinel) must be a valid
/// `extern "C" fn(i32)` pointer that lives at least until it is replaced.
pub unsafe fn signal(sig: i32, handler: Sighandler) -> Result<Sighandler, Errno> {
    // x86_64 must supply a restorer via SA_RESTORER; aarch64's kernel provides
    // one through the vDSO, so it sets neither the flag nor sa_restorer.
    #[cfg(target_arch = "x86_64")]
    let (sa_flags, sa_restorer) = (
        SA_RESTART | SA_RESTORER,
        sigreturn_trampoline as *const () as usize,
    );
    #[cfg(target_arch = "aarch64")]
    let (sa_flags, sa_restorer) = (SA_RESTART, 0usize);

    let new = KernelSigaction {
        sa_handler: handler,
        sa_flags,
        sa_restorer,
        sa_mask: 0,
    };
    let mut old = KernelSigaction {
        sa_handler: 0,
        sa_flags: 0,
        sa_restorer: 0,
        sa_mask: 0,
    };
    // rt_sigaction(sig, &new, &old, sigsetsize).
    // SAFETY: both pointers are valid, correctly-sized kernel sigaction
    // structs; `sigsetsize` matches the kernel's 64-bit sigset.
    let ret = unsafe {
        syscall4(
            nr::RT_SIGACTION,
            sig as usize,
            &new as *const KernelSigaction as usize,
            &mut old as *mut KernelSigaction as usize,
            SIGSETSIZE,
        )
    };
    from_ret(ret)?;
    Ok(old.sa_handler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{syscall0, syscall3};
    use core::sync::atomic::{AtomicU64, Ordering};

    // gettid/tgkill numbers differ per arch and aren't part of the crate's
    // public surface, so define them locally for the tests.
    #[cfg(target_arch = "x86_64")]
    const NR_GETTID: usize = 186;
    #[cfg(target_arch = "x86_64")]
    const NR_TGKILL: usize = 234;
    #[cfg(target_arch = "aarch64")]
    const NR_GETTID: usize = 178;
    #[cfg(target_arch = "aarch64")]
    const NR_TGKILL: usize = 131;

    // Direct the signal at the *current thread* so delivery is synchronous
    // (it completes before the raising syscall returns), giving exact counts
    // regardless of what the rest of the test harness's threads are doing.
    fn gettid() -> i32 {
        // SAFETY: gettid takes no args and never fails.
        unsafe { syscall0(NR_GETTID) as i32 }
    }
    fn tgkill(tgid: i32, tid: i32, sig: i32) {
        // SAFETY: plain integer arguments.
        unsafe { syscall3(NR_TGKILL, tgid as usize, tid as usize, sig as usize) };
    }

    static COUNT: AtomicU64 = AtomicU64::new(0);
    extern "C" fn counting_handler(_sig: i32) {
        COUNT.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn install_returns_previous_handler() {
        // Fresh disposition for SIGURG (unused here) is the default.
        let prev = unsafe { signal(SIGURG, SIG_IGN) }.expect("install IGN");
        assert_eq!(prev, SIG_DFL);
        // Reinstalling reports the SIG_IGN we just set.
        let prev = unsafe { signal(SIGURG, SIG_DFL) }.expect("install DFL");
        assert_eq!(prev, SIG_IGN);
    }

    #[test]
    fn invalid_signal_is_einval() {
        // SIGKILL cannot be caught: rt_sigaction rejects it with EINVAL.
        assert_eq!(unsafe { signal(SIGKILL, SIG_IGN) }, Err(Errno(22)));
    }

    // Signal dispositions and `COUNT` are process-global, and `cargo test`
    // runs tests in parallel threads, so all signal *delivery* assertions live
    // in this one sequential test to avoid cross-test races on that shared
    // state. (The disposition-only tests above touch neither `COUNT` nor these
    // signals, so they are safe to run concurrently.)
    #[test]
    fn delivery_stress_and_restorer_and_ignore() {
        let (pid, tid) = (crate::process::getpid(), gettid());

        // --- Storm: the crux of Phase 3. If the SA_RESTORER trampoline were
        // wrong, returning from the handler would fault. Delivering the signal
        // thousands of times to *this* thread (synchronous delivery) and
        // surviving with an exact count proves the trampoline and that no
        // deliveries are missed. ---
        COUNT.store(0, Ordering::SeqCst);
        let prev =
            unsafe { signal(SIGUSR1, counting_handler as *const () as usize) }.expect("install");
        const N: u64 = 5000;
        for _ in 0..N {
            tgkill(pid, tid, SIGUSR1);
        }
        assert_eq!(COUNT.load(Ordering::SeqCst), N);
        unsafe { signal(SIGUSR1, prev) }.expect("restore SIGUSR1");

        // --- Ignore: a SIG_IGN'd signal must not reach the handler. ---
        COUNT.store(0, Ordering::SeqCst);
        let prev = unsafe { signal(SIGUSR2, SIG_IGN) }.expect("ignore SIGUSR2");
        tgkill(pid, tid, SIGUSR2);
        assert_eq!(COUNT.load(Ordering::SeqCst), 0);
        unsafe { signal(SIGUSR2, prev) }.expect("restore SIGUSR2");
    }
}
