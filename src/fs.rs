//! Filesystem operations: file metadata via `statx`, access checks via
//! `faccessat`, and the path-mutating `*at` syscalls (`unlinkat`, `mkdirat`,
//! `renameat2`, `symlinkat`, `readlinkat`).
//!
//! Every entry point is an `*at`-form syscall taking an explicit `dirfd`;
//! [`AT_FDCWD`] resolves relative paths against the current working directory,
//! and thin `unlink`/`mkdir`/`rename`/`stat`/… wrappers pass it for you.
//! aarch64 has no legacy `stat`/`unlink`/`rename`, so building on the `*at`
//! forms keeps both arches identical.

use crate::arch::nr;
use crate::arch::{from_ret, syscall3, syscall4, syscall5, Errno};
use core::ffi::CStr;

pub use crate::fd::AT_FDCWD;

/// `*at` flag: do not follow a terminal symlink (operate on the link itself).
pub const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
/// `*at` flag: operate on `dirfd` itself when the path is empty (e.g. `fstat`
/// via [`statx`] on an open fd).
pub const AT_EMPTY_PATH: i32 = 0x1000;
/// [`unlinkat`] flag: remove a directory instead of a file (like `rmdir`).
pub const AT_REMOVEDIR: i32 = 0x200;

// --- access(2) mode bits ------------------------------------------------------

/// [`faccessat`] mode: test for existence only.
pub const F_OK: i32 = 0;
/// [`faccessat`] mode: test for execute (or directory search) permission.
pub const X_OK: i32 = 1;
/// [`faccessat`] mode: test for write permission.
pub const W_OK: i32 = 2;
/// [`faccessat`] mode: test for read permission.
pub const R_OK: i32 = 4;

/// Check the calling process's permissions for the file at `path` relative to
/// `dirfd`. `mode` is [`F_OK`] or an OR of [`R_OK`]/[`W_OK`]/[`X_OK`]; returns
/// `Ok(())` if all requested accesses are allowed, else the `Errno` (e.g.
/// `EACCES`, `ENOENT`).
///
/// The check uses the **real** uid/gid (matching the bare `faccessat` syscall),
/// which is exactly what a shell wants when testing PATH candidates for
/// executability.
pub fn faccessat(dirfd: i32, path: &CStr, mode: i32) -> Result<(), Errno> {
    // SAFETY: `path` is a valid nul-terminated C string the kernel only reads.
    let ret = unsafe {
        syscall3(
            nr::FACCESSAT,
            dirfd as usize,
            path.as_ptr() as usize,
            mode as usize,
        )
    };
    from_ret(ret).map(|_| ())
}

/// [`faccessat`] against the current working directory ([`AT_FDCWD`]).
pub fn access(path: &CStr, mode: i32) -> Result<(), Errno> {
    faccessat(AT_FDCWD, path, mode)
}

// --- statx(2) -----------------------------------------------------------------

/// `mask` bit set: request the basic `stat`-equivalent fields (mode, size,
/// timestamps, ids, links). The common choice for [`statx`].
pub const STATX_BASIC_STATS: u32 = 0x0000_07ff;

/// `stx_mode` type mask; AND with it and compare to an `S_IF*` value.
pub const S_IFMT: u16 = 0o170000;
/// `stx_mode` type: named pipe (FIFO).
pub const S_IFIFO: u16 = 0o010000;
/// `stx_mode` type: character device.
pub const S_IFCHR: u16 = 0o020000;
/// `stx_mode` type: directory.
pub const S_IFDIR: u16 = 0o040000;
/// `stx_mode` type: block device.
pub const S_IFBLK: u16 = 0o060000;
/// `stx_mode` type: regular file.
pub const S_IFREG: u16 = 0o100000;
/// `stx_mode` type: symbolic link.
pub const S_IFLNK: u16 = 0o120000;
/// `stx_mode` type: socket.
pub const S_IFSOCK: u16 = 0o140000;

/// A `statx` timestamp (kernel `struct statx_timestamp`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatxTimestamp {
    /// Seconds since the Unix epoch.
    pub tv_sec: i64,
    /// Nanoseconds within the second.
    pub tv_nsec: u32,
    __reserved: i32,
}

/// File metadata (kernel `struct statx`, 256 bytes). Only the fields requested
/// in the `statx` `mask` and reported back in [`Statx::stx_mask`] are valid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statx {
    /// Bitmask of which fields the kernel actually filled in.
    pub stx_mask: u32,
    /// Preferred I/O block size.
    pub stx_blksize: u32,
    /// Extra file attribute flags (`STATX_ATTR_*`).
    pub stx_attributes: u64,
    /// Number of hard links.
    pub stx_nlink: u32,
    /// Owner user ID.
    pub stx_uid: u32,
    /// Owner group ID.
    pub stx_gid: u32,
    /// File type and mode bits (test with [`S_IFMT`]/`S_IF*`).
    pub stx_mode: u16,
    __spare0: u16,
    /// Inode number.
    pub stx_ino: u64,
    /// Size in bytes (for a regular file, its length).
    pub stx_size: u64,
    /// Number of 512-byte blocks allocated.
    pub stx_blocks: u64,
    /// Which `stx_attributes` bits are supported/meaningful.
    pub stx_attributes_mask: u64,
    /// Last access time.
    pub stx_atime: StatxTimestamp,
    /// Creation (birth) time, if supported.
    pub stx_btime: StatxTimestamp,
    /// Last status-change time.
    pub stx_ctime: StatxTimestamp,
    /// Last modification time.
    pub stx_mtime: StatxTimestamp,
    /// Device major of the file this refers to (for device files).
    pub stx_rdev_major: u32,
    /// Device minor of the file this refers to (for device files).
    pub stx_rdev_minor: u32,
    /// Major of the device that holds the file.
    pub stx_dev_major: u32,
    /// Minor of the device that holds the file.
    pub stx_dev_minor: u32,
    /// Mount ID of the mount holding the file.
    pub stx_mnt_id: u64,
    /// Memory-alignment requirement for direct I/O, if any.
    pub stx_dio_mem_align: u32,
    /// Offset-alignment requirement for direct I/O, if any.
    pub stx_dio_offset_align: u32,
    __spare3: [u64; 12],
}

const _: () = assert!(core::mem::size_of::<StatxTimestamp>() == 16);
const _: () = assert!(core::mem::size_of::<Statx>() == 256);
const _: () = assert!(core::mem::offset_of!(Statx, stx_mode) == 28);
const _: () = assert!(core::mem::offset_of!(Statx, stx_ino) == 32);
const _: () = assert!(core::mem::offset_of!(Statx, stx_size) == 40);
const _: () = assert!(core::mem::offset_of!(Statx, stx_mtime) == 112);

impl Statx {
    /// The file-type portion of [`stx_mode`](Self::stx_mode).
    #[inline]
    pub const fn file_type(self) -> u16 {
        self.stx_mode & S_IFMT
    }
    /// True if this is a regular file.
    #[inline]
    pub const fn is_file(self) -> bool {
        self.file_type() == S_IFREG
    }
    /// True if this is a directory.
    #[inline]
    pub const fn is_dir(self) -> bool {
        self.file_type() == S_IFDIR
    }
    /// True if this is a symbolic link (only meaningful under
    /// [`AT_SYMLINK_NOFOLLOW`]/[`lstat`]).
    #[inline]
    pub const fn is_symlink(self) -> bool {
        self.file_type() == S_IFLNK
    }
}

/// Fetch metadata for `path` relative to `dirfd`. `flags` accepts
/// [`AT_SYMLINK_NOFOLLOW`]/[`AT_EMPTY_PATH`]; `mask` selects the fields to
/// retrieve (usually [`STATX_BASIC_STATS`]). See [`stat`]/[`lstat`]/[`fstat`]
/// for the common shorthands.
pub fn statx(dirfd: i32, path: &CStr, flags: i32, mask: u32) -> Result<Statx, Errno> {
    let mut buf = Statx::default();
    // statx(dirfd, path, flags, mask, &mut buf).
    // SAFETY: `path` is a valid C string; `buf` is a valid, exclusively
    // borrowed `struct statx` the kernel writes.
    let ret = unsafe {
        syscall5(
            nr::STATX,
            dirfd as usize,
            path.as_ptr() as usize,
            flags as usize,
            mask as usize,
            &mut buf as *mut Statx as usize,
        )
    };
    from_ret(ret)?;
    Ok(buf)
}

/// Metadata for `path`, following symlinks (like `stat(2)`).
pub fn stat(path: &CStr) -> Result<Statx, Errno> {
    statx(AT_FDCWD, path, 0, STATX_BASIC_STATS)
}

/// Metadata for `path` **without** following a terminal symlink (like
/// `lstat(2)`); [`Statx::is_symlink`] is meaningful here.
pub fn lstat(path: &CStr) -> Result<Statx, Errno> {
    statx(AT_FDCWD, path, AT_SYMLINK_NOFOLLOW, STATX_BASIC_STATS)
}

/// Metadata for an already-open descriptor (like `fstat(2)`), via
/// [`AT_EMPTY_PATH`].
pub fn fstat(fd: i32) -> Result<Statx, Errno> {
    statx(fd, c"", AT_EMPTY_PATH, STATX_BASIC_STATS)
}

// --- path mutations -----------------------------------------------------------

/// Remove the link at `path` relative to `dirfd`. `flags` may be `0` (unlink a
/// file) or [`AT_REMOVEDIR`] (remove an empty directory).
pub fn unlinkat(dirfd: i32, path: &CStr, flags: i32) -> Result<(), Errno> {
    // SAFETY: `path` is a valid C string the kernel only reads.
    let ret = unsafe {
        syscall3(
            nr::UNLINKAT,
            dirfd as usize,
            path.as_ptr() as usize,
            flags as usize,
        )
    };
    from_ret(ret).map(|_| ())
}

/// Delete the file `path` (relative to the cwd). Shorthand for [`unlinkat`].
pub fn unlink(path: &CStr) -> Result<(), Errno> {
    unlinkat(AT_FDCWD, path, 0)
}

/// Remove the empty directory `path`. Shorthand for [`unlinkat`] with
/// [`AT_REMOVEDIR`].
pub fn rmdir(path: &CStr) -> Result<(), Errno> {
    unlinkat(AT_FDCWD, path, AT_REMOVEDIR)
}

/// Create a directory `path` relative to `dirfd` with permission bits `mode`
/// (masked by the process umask).
pub fn mkdirat(dirfd: i32, path: &CStr, mode: u32) -> Result<(), Errno> {
    // SAFETY: `path` is a valid C string the kernel only reads.
    let ret = unsafe {
        syscall3(
            nr::MKDIRAT,
            dirfd as usize,
            path.as_ptr() as usize,
            mode as usize,
        )
    };
    from_ret(ret).map(|_| ())
}

/// Create the directory `path` (relative to the cwd). Shorthand for
/// [`mkdirat`].
pub fn mkdir(path: &CStr, mode: u32) -> Result<(), Errno> {
    mkdirat(AT_FDCWD, path, mode)
}

/// [`renameat2`] flag: fail with `EEXIST` if `new` already exists.
pub const RENAME_NOREPLACE: u32 = 1;
/// [`renameat2`] flag: atomically exchange `old` and `new` (both must exist).
pub const RENAME_EXCHANGE: u32 = 2;

/// Rename `old` (relative to `olddirfd`) to `new` (relative to `newdirfd`),
/// with `RENAME_*` `flags` (`0` for plain rename).
pub fn renameat2(
    olddirfd: i32,
    old: &CStr,
    newdirfd: i32,
    new: &CStr,
    flags: u32,
) -> Result<(), Errno> {
    // SAFETY: both paths are valid C strings the kernel only reads.
    let ret = unsafe {
        syscall5(
            nr::RENAMEAT2,
            olddirfd as usize,
            old.as_ptr() as usize,
            newdirfd as usize,
            new.as_ptr() as usize,
            flags as usize,
        )
    };
    from_ret(ret).map(|_| ())
}

/// Rename `old` to `new` (both relative to the cwd). Shorthand for
/// [`renameat2`] with no flags.
pub fn rename(old: &CStr, new: &CStr) -> Result<(), Errno> {
    renameat2(AT_FDCWD, old, AT_FDCWD, new, 0)
}

/// Create a symbolic link at `linkpath` (relative to `newdirfd`) pointing at
/// `target` (stored verbatim, not resolved).
pub fn symlinkat(target: &CStr, newdirfd: i32, linkpath: &CStr) -> Result<(), Errno> {
    // SAFETY: both paths are valid C strings the kernel only reads.
    let ret = unsafe {
        syscall3(
            nr::SYMLINKAT,
            target.as_ptr() as usize,
            newdirfd as usize,
            linkpath.as_ptr() as usize,
        )
    };
    from_ret(ret).map(|_| ())
}

/// Create the symlink `linkpath` -> `target` (relative to the cwd).
pub fn symlink(target: &CStr, linkpath: &CStr) -> Result<(), Errno> {
    symlinkat(target, AT_FDCWD, linkpath)
}

/// Read the target of the symlink at `path` (relative to `dirfd`) into `buf`,
/// returning the target bytes actually read.
///
/// `readlink` does **not** nul-terminate, and it truncates silently if `buf`
/// is too small — a returned length equal to `buf.len()` means the target may
/// have been longer, so retry with a bigger buffer.
pub fn readlinkat<'a>(dirfd: i32, path: &CStr, buf: &'a mut [u8]) -> Result<&'a [u8], Errno> {
    // SAFETY: `path` is a valid C string; `buf` is a valid, exclusively
    // borrowed slice the kernel writes at most `buf.len()` bytes into.
    let ret = unsafe {
        syscall4(
            nr::READLINKAT,
            dirfd as usize,
            path.as_ptr() as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
        )
    };
    let n = from_ret(ret)?;
    Ok(&buf[..n])
}

/// Read the target of the symlink `path` (relative to the cwd). Shorthand for
/// [`readlinkat`].
pub fn readlink<'a>(path: &CStr, buf: &'a mut [u8]) -> Result<&'a [u8], Errno> {
    readlinkat(AT_FDCWD, path, buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd;

    fn temp_path(tag: &str) -> std::ffi::CString {
        let p = format!(
            "{}/rusty_libc_fs_{}_{}",
            std::env::temp_dir().display(),
            tag,
            std::process::id()
        );
        std::ffi::CString::new(p).unwrap()
    }

    #[test]
    fn access_checks_existence_and_exec() {
        // /bin/sh exists and is executable; a bogus path does not exist.
        assert!(access(c"/bin/sh", F_OK).is_ok());
        assert!(access(c"/bin/sh", X_OK).is_ok());
        assert_eq!(
            access(c"/no/such/rusty_libc/path", F_OK),
            Err(Errno::ENOENT)
        );
    }

    #[test]
    fn statx_reports_type_and_size() {
        // A directory.
        let d = stat(c"/").expect("stat /");
        assert!(d.is_dir());
        assert!(!d.is_file());

        // A regular file we create with known contents.
        let path = temp_path("statx");
        let f = fd::open(&path, fd::O_WRONLY | fd::O_CREAT | fd::O_TRUNC, 0o600).expect("open");
        fd::write_all(f, b"12345").expect("write");

        // fstat via AT_EMPTY_PATH sees the size before we close.
        let via_fd = fstat(f).expect("fstat");
        assert!(via_fd.is_file());
        assert_eq!(via_fd.stx_size, 5);
        fd::close(f).expect("close");

        // stat by path agrees.
        let via_path = stat(&path).expect("stat file");
        assert!(via_path.is_file());
        assert_eq!(via_path.stx_size, 5);

        unlink(&path).expect("unlink");
    }

    #[test]
    fn symlink_lstat_readlink_unlink() {
        let link = temp_path("link");
        // Point it at a target that need not exist; symlink stores it verbatim.
        symlink(c"/target/of/link", &link).expect("symlink");

        // lstat sees the link itself.
        let st = lstat(&link).expect("lstat");
        assert!(st.is_symlink());

        // readlink returns the stored target (no NUL).
        let mut buf = [0u8; 256];
        let target = readlink(&link, &mut buf).expect("readlink");
        assert_eq!(target, b"/target/of/link");

        unlink(&link).expect("unlink link");
        assert_eq!(lstat(&link), Err(Errno::ENOENT));
    }

    #[test]
    fn mkdir_rename_rmdir() {
        let a = temp_path("dir_a");
        let b = temp_path("dir_b");
        // Clean any leftovers from a previous aborted run.
        let _ = rmdir(&a);
        let _ = rmdir(&b);

        mkdir(&a, 0o700).expect("mkdir a");
        assert!(stat(&a).expect("stat a").is_dir());

        rename(&a, &b).expect("rename a -> b");
        assert_eq!(stat(&a), Err(Errno::ENOENT));
        assert!(stat(&b).expect("stat b").is_dir());

        rmdir(&b).expect("rmdir b");
        assert_eq!(stat(&b), Err(Errno::ENOENT));
    }
}
