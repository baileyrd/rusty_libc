//! Resource limits via `prlimit64` (used for both get and set, `pid = 0`).

use crate::arch::nr;
use crate::arch::{from_ret, syscall4, Errno};

/// "No limit" sentinel for a [`Rlimit`] field.
pub const RLIM_INFINITY: u64 = u64::MAX;

// RLIMIT_* resource identifiers (asm-generic).
/// CPU time, in seconds.
pub const RLIMIT_CPU: i32 = 0;
/// Maximum file size.
pub const RLIMIT_FSIZE: i32 = 1;
/// Maximum data-segment size.
pub const RLIMIT_DATA: i32 = 2;
/// Maximum stack size.
pub const RLIMIT_STACK: i32 = 3;
/// Maximum core-dump size.
pub const RLIMIT_CORE: i32 = 4;
/// Maximum resident set size.
pub const RLIMIT_RSS: i32 = 5;
/// Maximum number of processes.
pub const RLIMIT_NPROC: i32 = 6;
/// Maximum number of open files.
pub const RLIMIT_NOFILE: i32 = 7;
/// Maximum lockable memory.
pub const RLIMIT_MEMLOCK: i32 = 8;
/// Maximum address-space size.
pub const RLIMIT_AS: i32 = 9;
/// Maximum number of file locks.
pub const RLIMIT_LOCKS: i32 = 10;
/// Maximum number of pending signals.
pub const RLIMIT_SIGPENDING: i32 = 11;
/// Maximum bytes in POSIX message queues.
pub const RLIMIT_MSGQUEUE: i32 = 12;
/// Ceiling on the process's nice value.
pub const RLIMIT_NICE: i32 = 13;
/// Ceiling on the real-time priority.
pub const RLIMIT_RTPRIO: i32 = 14;

/// A soft/hard resource-limit pair (kernel `struct rlimit64`; both fields are
/// always `u64`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rlimit {
    /// Soft limit (the value the kernel enforces).
    pub cur: u64,
    /// Hard limit (ceiling on the soft limit; only privileged callers raise
    /// it).
    pub max: u64,
}

const _: () = assert!(core::mem::size_of::<Rlimit>() == 16);

/// Get the current soft/hard limit for `resource` (a `RLIMIT_*` constant).
pub fn getrlimit(resource: i32) -> Result<Rlimit, Errno> {
    let mut old = Rlimit { cur: 0, max: 0 };
    // prlimit64(pid=0, resource, new=NULL, old=&mut).
    // SAFETY: `new` is null; `old` is a valid `*mut rlimit64`.
    let ret = unsafe {
        syscall4(
            nr::PRLIMIT64,
            0,
            resource as usize,
            0,
            &mut old as *mut Rlimit as usize,
        )
    };
    from_ret(ret)?;
    Ok(old)
}

/// Set the soft/hard limit for `resource` to `limit`.
pub fn setrlimit(resource: i32, limit: &Rlimit) -> Result<(), Errno> {
    // prlimit64(pid=0, resource, new=&, old=NULL).
    // SAFETY: `new` is a valid `*const rlimit64` the kernel only reads; `old`
    // is null.
    let ret = unsafe {
        syscall4(
            nr::PRLIMIT64,
            0,
            resource as usize,
            limit as *const Rlimit as usize,
            0,
        )
    };
    from_ret(ret).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_restore_nofile() {
        let original = getrlimit(RLIMIT_NOFILE).expect("getrlimit");

        // Lower the soft limit (always permitted up to the hard limit).
        let lowered = Rlimit {
            cur: original.cur.min(64),
            max: original.max,
        };
        setrlimit(RLIMIT_NOFILE, &lowered).expect("setrlimit lower");
        assert_eq!(getrlimit(RLIMIT_NOFILE).unwrap().cur, lowered.cur);

        // Restore.
        setrlimit(RLIMIT_NOFILE, &original).expect("setrlimit restore");
        assert_eq!(getrlimit(RLIMIT_NOFILE).unwrap(), original);
    }

    #[test]
    fn bad_resource_is_einval() {
        assert_eq!(getrlimit(9999), Err(Errno(22))); // EINVAL
    }
}
