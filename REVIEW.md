# rusty_libc — implementation review

Reviewed at `65f467f` (Phase 5 complete: x86_64 + aarch64, two-arch CI).

The crate is in good shape: clean `syscallN` stubs on both arches, an `Errno`
newtype with a correct `-4095..-1` decode window, kernel-layout structs guarded
by `const _:` size/offset assertions, and solid unit + PTY + signal-storm tests.
The items below are **additions and improvements**, ordered roughly by impact for
the consumer (rush, an interactive job-control shell). None are blockers for what
already ships.

## A. Missing primitives a shell actually needs

> **Status:** items 1–5 are implemented on this branch.

1. **`execve` / `execveat` (highest priority).** *(done)* Phase 4 added a raw `fork`, but
   there is no raw `exec`. A raw `fork` with no raw `exec` cannot launch an
   external command — the child can only `exit_group`. Every non-builtin a shell
   runs needs `execve(path, argv, envp)`. This is the single biggest functional
   gap now that `fork` is off glibc. Add `execve` (and ideally `execveat` for
   fd-relative exec), taking nul-terminated C strings and null-terminated
   pointer arrays.

2. **`open` / `openat` + `O_*` flags.** *(done)* The crate can only obtain fds from
   `pipe2` and `memfd_create`. File redirections (`>`, `>>`, `<`, `2>`,
   `2>&1`) require opening real files. Add `openat(AT_FDCWD, path, flags, mode)`
   with the `O_RDONLY/O_WRONLY/O_RDWR/O_CREAT/O_APPEND/O_TRUNC/O_CLOEXEC/
   O_NONBLOCK` constants.

3. **`rt_sigprocmask` (block / unblock / setmask).** *(done)* A job-control shell must
   block `SIGCHLD` (and often `SIGINT`/`SIGTSTP`) around critical sections —
   e.g. between forking and recording the child in the job table — then unblock.
   Today only `signal()` (disposition) is exposed; there is no way to mask.
   Add `sigprocmask(how, &new, Option<&mut old>)` over `rt_sigprocmask` with
   `sigsetsize = 8` and `SIG_BLOCK/SIG_UNBLOCK/SIG_SETMASK`.

4. **`chdir` / `fchdir` / `getcwd`.** *(done)* The `cd` builtin and `$PWD` tracking need
   these; none are present.

5. **Session / process-group control: `setsid`, `getpgid`, `getsid`.** *(done)* Job
   control and daemonizing need `setsid`; `setpgid` alone is not enough.

## B. Ergonomics and interop

> **Status:** items 6–12 are implemented on this branch.

6. **Named `Errno` constants.** *(done)* Callers and tests currently compare against
   magic numbers (`Errno(9)`, `Errno(22)`, `Errno(25)`, `Errno(10)`). Add
   `EPERM/EINTR/EBADF/EAGAIN/ECHILD/EINVAL/ENOTTY/ENOENT/EACCES/EEXIST/EPIPE/…`
   as `pub const Errno` values (e.g. `impl Errno { pub const EBADF: Errno =
   Errno(9); }`), then rewrite the tests to use them. Makes call-site error
   handling (retry-on-`EINTR`, ignore-`ECHILD`) readable.

7. **`Display` + `core::error::Error` for `Errno`.** *(done)* `core::error::Error` has
   been in `core` since Rust 1.81, so a `no_std` crate can implement it. Add a
   `Display` that prints the symbolic name (and, behind an optional `std`
   feature, `From<Errno> for std::io::Error` so consumers get
   `io::Error::from(errno)` directly instead of hand-rolling
   `from_raw_os_error`).

8. **Full `POLL*` flag set + `PollFd` helpers.** *(done)* Only `POLLIN` is exported. A
   shell reading from a pipe needs `POLLHUP` to notice the writer closing, and
   `POLLERR`/`POLLNVAL` to detect broken fds; `POLLOUT`/`POLLPRI` round it out.
   Add them and convenience methods like `PollFd::is_readable()` /
   `is_hup()` on `revents`.

9. **`fcntl` file-status flags.** *(done)* Doc/behaviour only cover `F_GETFD`/`F_SETFD`.
   Add `F_GETFL`/`F_SETFL` (+ `O_NONBLOCK`) and `F_DUPFD_CLOEXEC` — toggling
   non-blocking mode and cloexec-preserving dup are common shell needs.

10. **`pipe2`/`dup2` flag constants.** *(done)* `pipe2(flags)` takes raw flags but the
    crate exports none, forcing callers to hardcode `0o2000000` for
    `O_CLOEXEC`. Export `O_CLOEXEC`/`O_NONBLOCK` (shared with item 2) so
    `pipe2(O_CLOEXEC)` reads cleanly.

11. **`read_all` / `write_all` helpers.** *(done)* Even with `SA_RESTART`, writes to
    pipes and terminals can be short, and reads can return fewer bytes than
    requested. A small loop that drains/fills a buffer (and treats `EINTR` as
    retry) removes a class of bugs from callers.

12. **`tcsetattr` action variants.** *(done)* `tcsetattr` hardcodes `TCSETSW`
    (drain = `TCSADRAIN`). Terminal restore on exit typically wants
    `TCSAFLUSH` (`TCSETSF`) to discard pending input, and non-blocking paths
    want `TCSANOW` (`TCSETS`). Add an `optional_actions` parameter or
    `tcsetattr_now`/`tcsetattr_flush` variants.

## C. Build, CI, and correctness hygiene

> **Status:** items 13–16 are implemented on this branch.

13. **Declare an MSRV.** *(done)* The crate uses `#[unsafe(naked)]` + `naked_asm!`
    (stable since Rust 1.88), `offset_of!` (1.77), and `c"…"` literals (1.77),
    so the effective MSRV is 1.88. Add `rust-version = "1.88"` to `Cargo.toml`
    and an MSRV job to CI so a future edit doesn't silently raise it.

14. **Tighten lints.** *(done)* Promote `unsafe_op_in_unsafe_fn` from `warn` to `deny`,
    add `#![deny(missing_docs)]` (the public surface is already almost fully
    documented), and add a `cargo doc --no-deps -D warnings` step to CI so
    doc-link rot is caught.

15. **Guard the real `no_std` build.** *(done)* Because the test harness pulls in `std`
    via `cfg(not(test))`, `cargo build` on `*-linux-gnu` doesn't prove the
    crate links with no `std`. Add a genuinely `no_std` smoke target to CI
    (e.g. build a tiny `#![no_std] #![no_main]` example, or
    `cargo build -Z build-std=core` against a `*-none` sanity target) so an
    accidental `std::`/`alloc::` reference fails CI instead of shipping.

16. **Make the fork test harness-safe.** *(done)* `fork_child_runs_and_is_reaped` forks
    inside the multithreaded `cargo test` harness — exactly the hazard the
    `fork` safety note warns about. It is careful (child touches only raw
    syscalls), but it is still technically unsound under parallelism. Either
    move fork/signal-delivery tests into a separate integration test run with
    `--test-threads=1`, or document the constraint in CI.

## Nits

> **Status:** all three nits are addressed on this branch.

- *(done)* `killpg(0, sig)` relies on `0.wrapping_neg() == 0` so it targets the
  caller's own group via `kill(0, sig)` — correct, but worth a one-line comment
  since it reads as an accident.
- *(done)* `getrlimit`/`setrlimit` are hardcoded to `pid = 0`. Exposing the `pid`
  parameter of `prlimit64` (add `prlimit(pid, …)`, keep the pid-0 convenience
  wrappers) is a cheap generalization.
- *(done)* Consider a `RLIMIT_NLIMITS`/pipe-buffer (`RLIMIT` for the pipe size is a
  fcntl `F_SETPIPE_SZ`, not an rlimit) note; the DESIGN table lists a "pipe"
  rlimit that has no kernel equivalent.

---

# Round 2 — 10 more improvements

> **Status:** all 10 (plus the README refresh) are implemented on this branch.

A second pass after items 1–16 + nits landed. These are the next-most-valuable
additions for rush (a shell), ordered roughly by impact. None overlap with the
above.

## D. Missing syscalls a shell still needs

17. **File metadata via `statx`** *(done)* (new `stat` module). The `test`/`[`/`[[`
    builtins (`-e -f -d -h -s -x`, size, `-nt`/`-ot` by mtime) and prompt/`ls`
    helpers all need `stat`. `statx` is the one metadata syscall present and
    identical in shape on both x86_64 and aarch64 (the legacy `stat`/`fstat`
    structs differ per-arch and aarch64 lacks bare `stat`), so a kernel
    `struct statx` + `statx(dirfd, path, flags, mask, &mut buf)` with
    `S_IF*` helpers is the portable choice. Highest-value gap.

18. **`faccessat`** *(done)* + `F_OK`/`R_OK`/`W_OK`/`X_OK` (and `AT_EACCESS`). PATH
    command resolution ("is this candidate executable?") and `[ -x ]`/`[ -r ]`
    need an access check that doesn't require opening the file. Small, high
    value — it's on the hot path of every external command lookup.

19. **Filesystem mutations: `unlinkat`, `mkdirat`, `renameat2`, `symlinkat`, *(done)*
    `readlinkat`** (in `fd` or a new `fs` module, all `*at` forms with
    `AT_FDCWD`). Needed for here-doc/temp-file cleanup, the `mkdir`/`rm`-ish
    builtins some shells ship, and `cd -P`/symlink resolution (`readlinkat`).
    `renameat2` (not legacy `rename`) because aarch64 has only the `*at`
    variants.

20. **`geteuid`, `getgid`, `getegid`** *(done)* (+ optionally `getgroups`) in `process`.
    `getuid` is currently alone; a shell needs the **effective** ids for the
    privilege-aware prompt (`#` vs `$`), the `id`/`groups` builtins, and
    permission decisions. Three one-line syscalls.

21. **`clock_gettime` + `nanosleep`** *(done)* (new `time` module). Drives `$SECONDS`/
    `EPOCHREALTIME`, `read -t <timeout>`, the `sleep` builtin, and command
    timing (`time`). Both are raw syscalls on both arches; `CLOCK_MONOTONIC`/
    `CLOCK_REALTIME` constants come with them. (A vDSO fast path is a later
    optimization; the raw syscall is correct and enough to start.)

## E. Signals & job control

22. **`sigaction`-style install exposing `sa_flags`** *(done)* + the `SA_*` constants
    (`SA_NOCLDSTOP`, `SA_NOCLDWAIT`, `SA_RESTART`, `SA_NODEFER`,
    `SA_RESETHAND`, `SA_SIGINFO`). `signal()` hardcodes `SA_RESTART`, but a job
    controller wants `SA_NOCLDSTOP` on `SIGCHLD` (no `SIGCHLD` for stops it
    already reaps via `wait`) and often wants `SIGINT` **without** `SA_RESTART`
    so a blocked `read` breaks with `EINTR` on Ctrl-C. Keep `signal()` as the
    BSD-semantics convenience; add `sigaction(sig, handler, flags)` alongside.

23. **`sigsuspend` (+ `sigpending`)** *(done)* in `signal`. The race-free "wait for a
    signal" primitive: block `SIGCHLD`, check job state, then `sigsuspend` with
    it unblocked — closes the window where a `SIGCHLD` arriving between the
    check and the wait would be lost. Pairs directly with the existing
    `sigprocmask`.

## F. Terminal control

24. **Full `c_cc` index constants** *(done)* in `termios`: `VINTR VQUIT VERASE VKILL
    VEOF VEOL VSTART VSTOP VSUSP VREPRINT VWERASE VLNEXT VDISCARD` (only `VMIN`
    and `VTIME` exist today). A line editor that displays or rebinds the
    special characters (`^C`, `^Z`, erase/kill, `stty`-style output) needs the
    whole set; they are just indices into the existing `c_cc` array.

25. **`tcflush` / `tcdrain`** *(done)* (via `TCFLSH` / `TCSBRK` ioctls). `tcflush(fd,
    TCIFLUSH)` to discard typed-ahead input after an interrupt, and `tcdrain`
    to wait for output to drain, are standard line-editor operations. Small,
    and they reuse the existing crate-internal `ioctl` shim.

26. **`dup3(oldfd, newfd, flags)` public** *(done)* in `fd`. `dup2` exists, but there is
    no way to duplicate a descriptor onto another **with `O_CLOEXEC` set
    atomically** — which redirections want so the new fd doesn't leak across
    the next `exec`. The aarch64 path already calls `dup3` internally; just
    expose it (x86_64 has `dup3` too) and document the cloexec-redirection use.

## G. Docs & polish

- *(done)* **README is stale** — it still opens "A **planned** … crate" and lists no
  implemented surface, though the crate now covers ~25 syscalls across 9
  modules with the `std` feature. Refresh it: status, a module/coverage table,
  a short usage example, and the `std` feature flag. Cheap, and it's the first
  thing a consumer reads.
- *(done)* **Further candidates** (not in the 10): `waitid` (peek a child's
  status with `WNOWAIT` without reaping — useful for job tables), `pread`/
  `pwrite`, `getpgrp()` as a `getpgid(0)` convenience, and
  `EWOULDBLOCK`/`EINPROGRESS` `Errno` aliases.

---

# Round 3 — capabilities assessment

> **Status:** open. This round is a fresh line-by-line review of every module
> against a job-control shell's actual needs, done after Round 2 and the
> Track P work (`getdents64`, `pidfd_open`) landed. None of items 27–39 are
> implemented on this branch; they are proposed additions, not a changelog.

The crate's core (syscall stubs, `Errno`, kernel-layout structs, exec/fork/
signals/job-control/filesystem) is complete and correct for what it already
covers. The gaps below are real missing capabilities or correctness edges
found by reading each module against what still isn't representable —
ordered roughly by impact.

## H. Process-creation safety and correctness

27. **`clone3(CLONE_PIDFD)`: atomic fork + pidfd, closing the pid-reuse race
    `pidfd_open` still has.** The current pattern is `fork()` followed by a
    *separate* `pidfd_open(child_pid)` call. Between those two syscalls the
    child can exit — under load, with a busy pid space, the kernel can recycle
    its pid before `pidfd_open` runs, so the call either fails with `ESRCH`
    (safe but surprising) or, in a worse ordering, resolves a pidfd for a
    *different* process that reused the number. `clone3` returns the pidfd
    atomically as part of process creation, closing the window entirely. This
    finishes what `pidfd_open` started; keep `fork` + `pidfd_open` as the
    fallback for kernels/paths that don't use `clone3`.

28. **A `vfork`-style clone (`CLONE_VFORK | CLONE_VM`) for the
    fork-then-immediately-exec case.** `fork`'s own doc comment names the real
    hazard: a raw `clone(SIGCHLD)` child can inherit an allocator lock held by
    another parent thread and deadlock before it reaches `exec`, so the safety
    note pushes "only call when effectively single-threaded" onto the caller.
    `CLONE_VFORK | CLONE_VM` avoids the hazard by construction — the child
    shares the parent's address space and the parent is suspended until the
    child calls `exec`/`_exit`, so there is no independently-mutable,
    COW-duplicated heap for a stray lock to be held against. This is the
    technique `posix_spawn` implementations use to make fork-exec safe from a
    multithreaded parent, and "spawn an external command" is the overwhelming
    majority of a shell's forks. A `vfork_exec`-shaped primitive (narrower than
    raw `fork` — never returning control to arbitrary caller code in the
    child) would let a consumer drop the "must be single-threaded at the fork
    point" constraint for exactly that path.

29. **`waitpid`/`waitid` discard `rusage`.** Both wrappers pass a null
    `struct rusage` to `wait4`/`waitid`, throwing away a finished child's
    user/system CPU time at the syscall boundary — data the kernel already
    computed for free. A `time` builtin (`real`/`user`/`sys`, bash-style) needs
    exactly this. Add a `Rusage` struct and an `Option<&mut Rusage>` parameter
    (or a `waitpid_rusage` sibling) instead of a new syscall.

## I. Filesystem completeness

30. **`fchmodat`/`chmod` and `fchownat`/`chown`.** `fs` has full path mutation
    (`unlinkat`/`mkdirat`/`renameat2`/`symlinkat`) but no permission or
    ownership changes at all — no way to back `chmod`/`chown` builtins, or
    even adjust a mode after creation beyond `open`'s `mode` argument. Both are
    `*at`-form syscalls, fitting the module's existing convention exactly.

31. **`utimensat`.** No way to set atime/mtime (`touch`, cache-invalidation,
    `make`-adjacent builtins). Kernel-native, nanosecond resolution, and the
    modern replacement for `utime`/`utimes`.

32. **`linkat`.** `fs` covers `symlinkat` but not hard links; `ln` (without
    `-s`) has no primitive to call.

33. **`ftruncate`.** `O_TRUNC` only truncates at open time; there is no way to
    resize an already-open descriptor.

## J. Credentials, scheduling, and lifecycle

34. **`getgroups`.** Flagged as a maybe in the original Round 2 write-up and
    never added. `id`/`groups` builtins and supplementary-group-aware
    permission checks need it; it's a one-shot syscall like the other
    credential getters already in `process`.

35. **`nice`/`getpriority`/`setpriority`.** No priority control at all — a
    `nice` builtin, or a shell that backgrounds low-priority jobs, has no
    primitive to call.

36. **`prctl(PR_SET_PDEATHSIG)`.** No `prctl` at all, so there is no path even
    for an opt-in caller to ask the kernel to signal a child if its parent
    dies first — the standard fix for orphaned children surviving a
    crashed/killed job-control shell.

## K. Time and completeness polish

37. **`alarm`/`setitimer` (or `timerfd_create`).** Nothing generates a
    timeout signal or fd; `TMOUT`/`read -t`-style features would have to
    busy-poll `clock_gettime` today. Given `poll` already exists,
    `timerfd_create` composes better than `SIGALRM` (one more fd in the
    existing poll loop, no signal-safety concerns) and is worth preferring
    over classic `alarm`.

38. **`uname`.** No system-identity syscall. Minor, but it's the primitive
    behind `$OSTYPE`/`$MACHTYPE`-style variables and is a single
    no-pointer-argument syscall.

39. **`readv`/`writev`.** `fd` has `read`/`write`/`pread`/`pwrite` but no
    scatter-gather. Not blocking anything today, but composes well with
    here-doc/redirection assembly that currently has to concatenate into one
    buffer first.

## L. Event-driven signals — a design question, not a gap

> **Status:** decided. See
> [ADR-0002](docs/adr/0002-signalfd-as-primary-event-driven-signal-path.md):
> `signalfd` is the primary recommended path for the asynchronous signals a
> job-control shell reacts to (`SIGCHLD`/`SIGINT`/`SIGWINCH`/`SIGTERM`/`SIGHUP`
> and `trap`-style user handlers), with `sigaction` kept for synchronous
> hardware-fault signals and handler-based compatibility. Implementation
> tracked separately (not part of this decision — see the ADR's
> "Consequences").

Not a missing primitive, a flag for a decision: the signal story (`sigaction`,
`sigprocmask`, `sigsuspend`) is complete and correct, but it's also the most
failure-prone part of anything built on it — signal-safety rules, the
mask-then-wait race this crate already had to close carefully. `signalfd`
would let a job-control shell fold `SIGCHLD`/`SIGINT`/`SIGWINCH` delivery into
the same `poll` loop it already uses for I/O instead of an async-signal-context
handler at all — no handler, no restorer, no signal-safety constraints on what
runs when a signal arrives. Given how much of this crate's hardest work (the
x86_64 restorer trampoline, the signal-storm stress test) exists purely to
make the handler path *correct*, it's worth asking whether `signalfd` should
become the primary recommended path for new consumers, with `sigaction` kept
for handler-based compatibility. Not numbered as a gap — flagging for a
decision, not proposing to silently add.

## Nits

- `Errno`'s named-constant set (23 consts) is narrower than what `name()`
  recognizes (~38 codes). The gap matters where it's most likely to bite: a
  filesystem-heavy consumer will want to match on `ENOSPC`, `EROFS`,
  `ENAMETOOLONG`, `ENOTEMPTY`, and `ELOOP` by name, and none of the five has a
  const today. Concretely: `process`'s own `getcwd` test already hardcodes
  `Err(Errno(34)) // ERANGE` — a magic number of exactly the kind Round 1
  item 6 eliminated everywhere else, because `ERANGE` has no named const.
- `execve`/`execveat` have no ergonomic way to build `argv`/`envp` from owned
  strings; every caller hand-rolls null-terminated pointer arrays. The no_std/
  no-alloc core is a deliberate constraint, so this isn't a core-crate fix,
  but the opt-in `std` feature (already the home of `Errno`'s `io::Error`
  interop) is the natural place for a small `ArgvBuilder`/`EnvBuilder`
  convenience — nothing forces it into the no_std path.
