# rusty_libc vs libc — micro-benchmark

A small, reproducible harness that issues the **same syscalls** through
`rusty_libc` and through the [`libc`](https://crates.io/crates/libc) crate
(glibc), and reports ns/op for each.

It is a **standalone crate** (its own `[workspace]`), intentionally kept out of
the library's build: running `cargo build`/`test`/`clippy` at the repo root
never compiles it, so the library keeps its zero-dependency guarantee — `libc`
appears only here.

## Run

```sh
cd bench
cargo run --release
```

Requires network access to fetch the `libc` dev-dependency and a Linux host
(the crate is Linux-only). Reports the best of several timed rounds to reduce
noise. Absolute numbers vary by machine/kernel (CPU mitigations inflate syscall
cost); the **ratio** column is the meaningful part.

## What it measures

| operation | notes |
|---|---|
| `getpid`, `getuid` | pure syscalls (modern glibc does not cache `getpid`) |
| `read(/dev/zero)`, `write(/dev/null)` | real I/O syscalls |
| `clock_gettime(MONOTONIC)` | served from the **vDSO** in userspace on both sides |

## Expected result

For genuine syscalls, `rusty_libc` and `libc` are at parity (±a few %): the
kernel trap dominates, and `rusty_libc`'s raw `asm!` stub only saves glibc's PLT
indirection and `errno` TLS write — a couple of nanoseconds, lost in the noise.
So swapping the `libc` crate for `rusty_libc` has no runtime cost.

`clock_gettime` is the one call the kernel accelerates via the vDSO (no syscall
trap). `rusty_libc` resolves and calls the vDSO entry too (see `src/vdso.rs`),
so it is ~an order of magnitude faster than a raw `clock_gettime` syscall and
lands within a few nanoseconds of glibc. Sample run (native x86_64):

```
operation               rusty (ns)    libc (ns)  rusty/libc
-----------------------------------------------------------
getpid                       247.33       247.99       1.00x
getuid                       252.48       252.52       1.00x
read(/dev/zero,64)           293.97       296.79       0.99x
write(/dev/null,64)          284.01       278.35       1.02x
clock_gettime(MONO)           26.21        20.85       1.26x
```
