//! # rusty_libc
//!
//! A `#![no_std]`, zero-dependency, Linux-only crate that replaces the `libc`
//! FFI-bindings crate for [rush](https://github.com/baileyrd/rush) by issuing
//! raw syscalls via inline asm instead of linking prototypes against glibc.
//!
//! See `DESIGN.md` for the required API surface, hard problems, phasing, and
//! testing strategy.
//!
//! ## Conventions
//!
//! - Safe wrappers return [`Result<T, Errno>`](arch::Errno); a raw `-errno`
//!   return value never escapes the crate. Callers needing a
//!   `std::io::Error` should use `Error::from_raw_os_error(errno.0)` — this
//!   crate does **not** write glibc's TLS `errno`, so `last_os_error()` is
//!   not meaningful after these calls.
//! - `unsafe` is confined to the [`arch`] module (asm) and the pointer-taking
//!   ioctl internals; the public API is safe except where inherently not.
//! - All struct layouts are the **kernel's**, checked by `const _:`
//!   size/offset assertions, never copied from glibc headers.

// Build as `no_std` normally, but let the built-in test harness pull in `std`
// so `cargo test` can run the unit tests.
#![cfg_attr(not(test), no_std)]
// The entire crate is Linux-only; on other targets it compiles to nothing.
#![cfg(target_os = "linux")]
