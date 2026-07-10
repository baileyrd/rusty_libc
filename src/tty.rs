//! Terminal window size query (`TIOCGWINSZ`).

use crate::arch::Errno;
use crate::fd::ioctl;

/// `ioctl` request: get the terminal window size.
const TIOCGWINSZ: usize = 0x5413;

/// Terminal dimensions. Kernel `struct winsize` layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Winsize {
    /// Rows, in characters.
    pub ws_row: u16,
    /// Columns, in characters.
    pub ws_col: u16,
    /// Width, in pixels (often 0).
    pub ws_xpixel: u16,
    /// Height, in pixels (often 0).
    pub ws_ypixel: u16,
}

const _: () = assert!(core::mem::size_of::<Winsize>() == 8);

/// Query the window size of the terminal referred to by `fd`.
pub fn window_size(fd: i32) -> Result<Winsize, Errno> {
    let mut ws = Winsize::default();
    // SAFETY: `TIOCGWINSZ` expects a `*mut winsize`; `ws` is exactly that and
    // exclusively borrowed.
    unsafe { ioctl(fd, TIOCGWINSZ, &mut ws as *mut Winsize as usize) }?;
    Ok(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_tty_fails() {
        // A pipe is not a terminal: expect ENOTTY (25).
        let (r, w) = crate::fd::pipe2(0).unwrap();
        assert_eq!(window_size(r), Err(Errno(25)));
        crate::fd::close(r).unwrap();
        crate::fd::close(w).unwrap();
    }
}
