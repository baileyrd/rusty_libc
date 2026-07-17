# rusty_libc vs libc — micro-benchmark

A small, reproducible harness that issues the **same syscalls** through
`rusty_libc` and through the [`libc`](https://crates.io/crates/libc) crate
(glibc), and reports ns/op for each.

It is a **standalone crate** (its own `[workspace]`), intentionally kept out of
the library's build: running `cargo build`/`test`/`clippy` at the repo root
never compiles it, so the library keeps its zero-dependency guarantee — `libc`
appears only here.

The `libc` crate binds the **system C library**, so the comparison target is
whichever libc the build links — **glibc** on the default `-gnu` target,
**musl** on the `-musl` target. The binary self-labels its output accordingly.

## Run

```sh
cd bench
cargo run --release                                    # vs glibc
cargo run --release --target x86_64-unknown-linux-musl # vs musl
./run.sh                                               # both
```

Requires network access to fetch the `libc` dev-dependency and a Linux host
(the crate is Linux-only). Reports the best of several timed rounds to reduce
noise. Absolute numbers vary by machine/kernel (CPU mitigations inflate syscall
cost); the **ratio** column is the meaningful part.

### musl

The musl target produces a **static** binary (Rust links a self-contained musl,
so no `musl-gcc` is needed). Add the target once:

```sh
rustup target add x86_64-unknown-linux-musl
```

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
trap). Both glibc and musl serve it from the vDSO, and `rusty_libc` resolves and
calls the vDSO entry too (see `../src/vdso.rs`), so it is ~an order of magnitude
faster than a raw `clock_gettime` syscall and lands within a few nanoseconds of
both. rusty_libc is at parity with **both** libcs. Sample run (native x86_64):

```
rusty_libc vs glibc  (best of 8 rounds x 1000000 iters)

operation                rusty (ns)   glibc (ns)  rusty/glibc
------------------------------------------------------------
getpid                       241.77       243.28        0.99x
getuid                       239.06       239.48        1.00x
read(/dev/zero,64)           288.79       290.15        1.00x
write(/dev/null,64)          269.52       272.95        0.99x
clock_gettime(MONO)           26.35        20.86        1.26x

rusty_libc vs musl  (best of 8 rounds x 1000000 iters)

operation                rusty (ns)    musl (ns)   rusty/musl
------------------------------------------------------------
getpid                       245.35       246.63        0.99x
getuid                       241.56       240.73        1.00x
read(/dev/zero,64)           287.21       292.20        0.98x
write(/dev/null,64)          275.53       271.69        1.01x
clock_gettime(MONO)           26.49        21.34        1.24x
```
