# rusty_libc

A planned `#![no_std]`, zero-dependency, Linux-only raw-syscall crate to
replace the `libc` FFI-bindings crate in [rush](https://github.com/baileyrd/rush).

See [DESIGN.md](DESIGN.md) for the required API surface, hard problems
(signal restorer trampoline, fork-vs-threads, kernel-vs-glibc struct
layouts), phasing, and testing strategy. The full dependency analysis that
motivates this lives in rush's `docs/LIBC_DEPENDENCY_ANALYSIS.md`.
