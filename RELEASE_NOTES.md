# Release Notes

Stay up to date with the latest changes to rusty_libc.

> rusty_libc has not yet been published to crates.io — it is still `0.0.1` and
> consumed by [rush](https://github.com/baileyrd/rush) directly off `main`.
> Everything below is merged; there is no unreleased/pending section because
> `main` **is** the release. Once the API is considered stable for outside
> consumers we'll cut a tagged `0.x` release and this section will split into
> "Unreleased" vs. tagged versions, the way most changelogs do.

---

## Inline the hot path — [PR #16](https://github.com/baileyrd/rusty_libc/pull/16)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/97c80c2...826019e)**

**Performance**
- Marked 15 thin convenience wrappers `#[inline]` — `open`, `access`,
  `stat`/`lstat`/`fstat`, `unlink`/`rmdir`, `mkdir`, `rename`, `symlink`,
  `readlink`, `killpg`, `getpgrp`, `getrlimit`/`setrlimit`, `tcsetattr`,
  `isatty`, `signal`/`sigaction`. Verified by disassembling a downstream
  consumer built **without** LTO: these functions previously survived as real,
  separate symbols reached through an indirect call, because rustc's automatic
  cross-crate inlining only reaches trivial single-hop leaves (like `getpid`)
  and not functions that forward to another wrapper. After the annotation they
  fully collapse into the caller.

---

## Coarse clocks + `clock_gettime` tuning — [PR #15](https://github.com/baileyrd/rusty_libc/pull/15)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/df46fdd...97c80c2)**

**Added**
- `time::CLOCK_REALTIME_COARSE`, `CLOCK_MONOTONIC_COARSE`, `CLOCK_BOOTTIME`.
  The `*_COARSE` clocks skip the vDSO's fine-grained interpolation, trading
  millisecond-ish resolution for speed — about 4x faster than the precise
  clock in this crate's own benchmark. The right choice for `$SECONDS`-style
  counters, throttling, or any hot-path timestamp that doesn't need sub-ms
  accuracy.

**Performance**
- `clock_gettime` now fills a `MaybeUninit<Timespec>` instead of
  zero-initializing, and is marked `#[inline]`.
- The vDSO function-pointer cache now uses `Ordering::Relaxed` instead of
  `Acquire`/`Release` — the cached value is a fixed vDSO code address the
  kernel already mapped executable before the process ran any code, so there
  was nothing for the fence to synchronize.

---

## Benchmark: add a musl comparison — [PR #14](https://github.com/baileyrd/rusty_libc/pull/14)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/7da292e...df46fdd)**

**Added**
- `bench/` now builds and runs against **musl** as well as glibc
  (`--target x86_64-unknown-linux-musl`, or `bench/run.sh` for both). The
  harness self-labels its output (`glibc` vs `musl`) from `target_env`, and
  the musl build is a static binary via Rust's self-contained musl linking —
  no `musl-gcc` required.

---

## A reproducible benchmark harness — [PR #13](https://github.com/baileyrd/rusty_libc/pull/13)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/dc466d3...7da292e)**

**Added**
- `bench/`: a standalone crate (its own `[workspace]`, kept out of the
  library's build) comparing rusty_libc against the `libc` crate on the same
  syscalls — `getpid`, `getuid`, `read`, `write`, `clock_gettime`. `libc` never
  enters the library's own dependency graph, so the zero-dependency guarantee
  holds.

---

## A vDSO fast path for `clock_gettime` — [PR #12](https://github.com/baileyrd/rusty_libc/pull/12)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/f0d08d1...dc466d3)**

**Performance**
- `time::clock_gettime` now resolves and calls the process vDSO's
  `clock_gettime` (reading `AT_SYSINFO_EHDR` from `/proc/self/auxv` and parsing
  the vDSO's ELF symbol table — no libc), matching glibc's speed instead of
  paying a syscall trap on every call. About 11x faster
  (~287ns → ~26ns), landing within a few ns of glibc's own vDSO call. Falls
  back to the raw syscall whenever the vDSO is unavailable, so it is a pure
  optimization, never a correctness dependency.

---

## More syscalls: `waitid`, `pread`/`pwrite`, `getpgrp`, errno aliases — [PR #11](https://github.com/baileyrd/rusty_libc/pull/11)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/d058f1f...f0d08d1)**

**Added**
- `wait::waitid` — peek a child's status via `WNOWAIT` **without** reaping it,
  returning a kernel `Siginfo`; plus `P_ALL`/`P_PID`/`P_PGID`,
  `WEXITED`/`WSTOPPED`, and the `CLD_*` reason codes.
- `fd::pread`/`fd::pwrite` — positioned I/O that doesn't move the file offset.
- `process::getpgrp()` — convenience for `getpgid(0)`.
- `Errno::EWOULDBLOCK` (alias of `EAGAIN`) and `Errno::EINPROGRESS`.

---

## Round 2: filesystem metadata, faccessat, time, signal flags, terminal control — [PR #10](https://github.com/baileyrd/rusty_libc/pull/10)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/4f04cc1...d058f1f)**

**Added**
- `fs` module: `statx` + `stat`/`lstat`/`fstat` (kernel-layout `Statx` with
  `is_dir`/`is_file`/`is_symlink`), `faccessat`/`access`
  (`F_OK`/`R_OK`/`W_OK`/`X_OK`), and the path mutators `unlinkat`/`mkdirat`/
  `renameat2`/`symlinkat`/`readlinkat` with `unlink`/`rmdir`/`mkdir`/`rename`/
  `symlink`/`readlink` shorthands.
- `time` module: `Timespec`, `clock_gettime`, `nanosleep`,
  `CLOCK_REALTIME`/`CLOCK_MONOTONIC`.
- `process::geteuid`/`getgid`/`getegid` — effective/real credential getters
  for privilege-aware prompts and permission checks.
- `signal::sigaction(sig, handler, flags)` with the full `SA_*` constant set
  (`signal()` keeps its simpler `SA_RESTART`-only BSD semantics), plus
  `sigpending` and `sigsuspend` — the race-free "wait for a signal" primitive.
- `termios`: the full `c_cc` index set (`VINTR`..`VEOL2`), `tcflush`/`tcdrain`.
- `fd::dup3(oldfd, newfd, flags)`, public, for atomic-`O_CLOEXEC`
  redirections.

**Changed**
- README rewritten: status, a per-module coverage table, a usage example, the
  `std` feature, and MSRV.

---

## Fix nits: `prlimit(pid, …)`, a `killpg` comment, pipe-size `fcntl` — [PR #9](https://github.com/baileyrd/rusty_libc/pull/9)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/92ab59b...4f04cc1)**

**Added**
- `rlimit::prlimit(pid, resource, new, old)` — the full `prlimit64` primitive
  (get/set/atomic-swap on any pid); `getrlimit`/`setrlimit` are now thin
  `pid = 0` wrappers over it.
- `fd::F_SETPIPE_SZ`/`F_GETPIPE_SZ` — the real pipe-capacity mechanism (Linux
  has no `RLIMIT` for it).

**Fixed**
- `killpg`'s `pgrp.wrapping_neg()` now has a comment explaining why it's
  correct (including the `pgrp == 0` → "caller's own group" case) instead of
  reading like an accident.

---

## Implementation review round 1: exec, filesystem, signals, session control, ergonomics, CI — [PR #8](https://github.com/baileyrd/rusty_libc/pull/8)

**July 17, 2026 • [Compare changes](https://github.com/baileyrd/rusty_libc/compare/65f467f...92ab59b)**

The first pass of a full implementation review (see
[REVIEW.md](REVIEW.md)) — sixteen additions closing the gap between "raw
syscalls exist" and "a shell can actually use this."

**Added**
- `process::execve`/`execveat` — raw exec, the piece that made a raw `fork`
  with no way to launch external commands.
- `fd::open`/`openat` + the `O_*` flag set — file redirections.
- `signal::sigprocmask` + `sigmask()` — block `SIGCHLD` around job-control
  critical sections.
- `process::chdir`/`fchdir`/`getcwd` — the `cd` builtin and `$PWD`.
- `process::setsid`/`getpgid`/`getsid` — session and process-group control.
- Named `Errno` constants (`EBADF`, `EINVAL`, `EAGAIN`, …) plus `Errno::name()`,
  `Display`, and `core::error::Error`; an opt-in `std` feature adds
  `From<Errno> for std::io::Error`.
- The full `POLL*` flag set and `PollFd` helper methods
  (`is_readable`/`is_hup`/…).
- `fcntl` `F_GETFL`/`F_SETFL`; `O_CLOEXEC`/`O_NONBLOCK` constants.
- `fd::read_all`/`write_all` — loop over short I/O, retrying on `EINTR`.
- `termios::tcsetattr_with` + `TCSANOW`/`TCSADRAIN`/`TCSAFLUSH`.

**Changed**
- MSRV declared (`rust-version = "1.88"`, the `naked_asm!` floor) with a CI
  job to catch regressions.
- `unsafe_op_in_unsafe_fn` and `missing_docs` promoted from warn to deny;
  `cargo doc -D warnings` added to CI.
- CI now builds `no_std` explicitly (`--no-default-features`) and the `std`
  feature separately, and runs tests single-threaded so the process is
  effectively single-threaded at each `fork` point.

---

## Foundational build — Phases 1–5

**July 10–11, 2026**

The initial implementation, before the review process above began.

- **Phase 1** — raw-syscall core: `syscall0`..`syscall6`, `Errno` decode,
  `fd` primitives (`read`/`poll`/`pipe2`/`dup`/`dup2`/`close`/`fcntl`),
  process ids and `setpgid`, `tcsetpgrp`/`isatty`, `rlimit`, `umask`.
- **Phase 2** — kernel-layout `Termios`, raw-mode transform, validated against
  a real PTY.
- **Phase 3** — signals via `rt_sigaction`, including the x86_64
  `SA_RESTORER` trampoline (a naked function issuing `rt_sigreturn`) — the
  hardest problem in the crate, stress-tested with a 5,000-signal delivery
  storm.
- **Phase 4** — raw `fork` (`clone(SIGCHLD)`), `memfd_create`, `write`,
  `lseek`.
- **Phase 5** — the aarch64 syscall table and a two-arch CI matrix
  (`x86_64` native, `aarch64` cross-compiled under `qemu-user`).
- `RLIMIT_RTTIME` constant.
