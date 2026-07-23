//! Memory mapping: `mmap`(2)/`munmap`(2)/`mprotect`(2).
//!
//! No caller in `rush` uses this today; it exists as a primitive for future
//! callers (a memory-mapped history file, or a large-buffer alternative to a
//! read loop) rather than a confirmed gap. See REVIEW.md Round 4, item 48.

use crate::arch::nr;
use crate::arch::{from_ret, syscall3, syscall6, Errno};

/// `mmap`/`mprotect` protection: pages may be read.
pub const PROT_READ: i32 = 0x1;
/// `mmap`/`mprotect` protection: pages may be written.
pub const PROT_WRITE: i32 = 0x2;
/// `mmap`/`mprotect` protection: pages may be executed.
pub const PROT_EXEC: i32 = 0x4;
/// `mmap`/`mprotect` protection: pages may not be accessed at all.
pub const PROT_NONE: i32 = 0x0;

/// `mmap` flag: changes are shared with other mappings of the same
/// file/region (and written back to the backing file, if any).
pub const MAP_SHARED: i32 = 0x01;
/// `mmap` flag: changes are private to this mapping (copy-on-write); never
/// written back to the backing file.
pub const MAP_PRIVATE: i32 = 0x02;
/// `mmap` flag: `addr` is not a hint -- map at exactly that address (or fail).
pub const MAP_FIXED: i32 = 0x10;
/// `mmap` flag: the mapping has no backing file; `fd` must be `-1` and
/// `offset` `0`. Combine with [`MAP_PRIVATE`] for a zero-initialized
/// scratch buffer.
pub const MAP_ANONYMOUS: i32 = 0x20;

/// Map `length` bytes starting at `offset` in `fd` (or an anonymous, unbacked
/// region when `flags` includes [`MAP_ANONYMOUS`]) into the calling
/// process's address space, returning the mapping's base address.
///
/// `addr` is a placement hint; pass `0` to let the kernel choose unless
/// `flags` includes [`MAP_FIXED`], in which case it is mandatory. `length`
/// and `offset` are rounded/must already be page-aligned per the kernel's
/// usual rules.
///
/// # Safety
///
/// The returned region is raw, uninitialized (or file-backed) memory outside
/// Rust's ownership model: the caller must not let it outlive an [`munmap`]
/// call, must not construct overlapping mappings that alias a `&mut`
/// elsewhere, and must respect `prot` (e.g. never write through a
/// `PROT_READ`-only mapping).
pub unsafe fn mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: i64,
) -> Result<*mut u8, Errno> {
    // mmap(addr, length, prot, flags, fd, offset).
    // SAFETY: forwarded to the caller's own safety contract; this wrapper
    // performs no dereference itself.
    let ret = unsafe {
        syscall6(
            nr::MMAP,
            addr,
            length,
            prot as usize,
            flags as usize,
            fd as usize,
            offset as usize,
        )
    };
    from_ret(ret).map(|p| p as *mut u8)
}

/// Unmap the `length`-byte region starting at `addr`, previously returned by
/// [`mmap`].
///
/// # Safety
///
/// `addr`/`length` must describe (a subset of) a live mapping this process
/// owns; every pointer into the unmapped range becomes dangling the instant
/// this returns `Ok`.
pub unsafe fn munmap(addr: *mut u8, length: usize) -> Result<(), Errno> {
    // munmap(addr, length).
    // SAFETY: forwarded to the caller's own safety contract.
    let ret = unsafe { syscall3(nr::MUNMAP, addr as usize, length, 0) };
    from_ret(ret).map(|_| ())
}

/// Change the protection of the `length`-byte region starting at `addr`
/// (a live mapping from [`mmap`]) to `prot`.
///
/// # Safety
///
/// `addr`/`length` must describe (a subset of) a live mapping this process
/// owns; narrowing `prot` (e.g. dropping [`PROT_WRITE`]) invalidates any
/// `&mut` borrow a caller may be holding into that range.
pub unsafe fn mprotect(addr: *mut u8, length: usize, prot: i32) -> Result<(), Errno> {
    // mprotect(addr, length, prot).
    // SAFETY: forwarded to the caller's own safety contract.
    let ret = unsafe { syscall3(nr::MPROTECT, addr as usize, length, prot as usize) };
    from_ret(ret).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: usize = 4096;

    #[test]
    fn anonymous_mapping_reads_zeroed_and_is_writable() {
        unsafe {
            let p = mmap(
                0,
                PAGE,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
            .expect("mmap");
            assert!(!p.is_null());

            let slice = core::slice::from_raw_parts(p, PAGE);
            assert!(slice.iter().all(|&b| b == 0));

            *p = 0xab;
            assert_eq!(*p, 0xab);

            munmap(p, PAGE).expect("munmap");
        }
    }

    #[test]
    fn mprotect_read_only_then_restore() {
        unsafe {
            let p = mmap(
                0,
                PAGE,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
            .expect("mmap");
            *p = 1;

            mprotect(p, PAGE, PROT_READ).expect("mprotect read-only");
            // Reading through the now-read-only mapping is still fine.
            assert_eq!(*p, 1);

            mprotect(p, PAGE, PROT_READ | PROT_WRITE).expect("mprotect restore");
            *p = 2;
            assert_eq!(*p, 2);

            munmap(p, PAGE).expect("munmap");
        }
    }

    #[test]
    fn mmap_bad_fd_without_anonymous_is_ebadf() {
        unsafe {
            assert_eq!(
                mmap(0, PAGE, PROT_READ, MAP_PRIVATE, -1, 0),
                Err(Errno::EBADF)
            );
        }
    }

    #[test]
    fn munmap_bad_length_is_einval() {
        unsafe {
            assert_eq!(
                munmap(core::ptr::dangling_mut::<u8>(), 0),
                Err(Errno::EINVAL)
            );
        }
    }

    #[test]
    fn mprotect_bad_addr_is_enomem() {
        unsafe {
            // A page-aligned address with no mapping behind it: create one,
            // free it, and reuse the (still page-aligned) address -- an
            // unaligned address would fail with EINVAL before the kernel
            // ever checks whether anything is mapped there.
            let p = mmap(
                0,
                PAGE,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
            .expect("mmap");
            munmap(p, PAGE).expect("munmap");

            assert_eq!(mprotect(p, PAGE, PROT_READ), Err(Errno::ENOMEM));
        }
    }
}
