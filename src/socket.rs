//! TCP/UDP sockets over IPv4 (`AF_INET`) and IPv6 (`AF_INET6`).
//!
//! Kernel `struct sockaddr_in`/`sockaddr_in6` layouts (checked by
//! `const` size/offset assertions, not copied from glibc), matching this
//! crate's usual convention. `port`/`flowinfo` are stored in network byte
//! order exactly as the kernel expects them -- [`SockAddrIn::new`]/
//! [`SockAddrIn6::new`] take a host-order `u16` port and do the conversion
//! for you. Address octets need no such conversion: they're already in
//! transmission order (`[127, 0, 0, 1]` for `127.0.0.1`), so they're stored
//! and read back as a plain byte array, not swapped through a numeric type.
//!
//! Blocking by default, like every other fd primitive in this crate: pass
//! [`SOCK_NONBLOCK`] to [`socket`] for non-blocking I/O instead of assuming
//! an async runtime. See [ADR-0003](../../docs/adr/0003-add-sockets-tcp-udp-and-dns-resolution.md)
//! for why this module exists and what it deliberately leaves out (no
//! `setsockopt`/`getsockopt`, no Unix-domain sockets -- nothing beyond what
//! `/dev/tcp`/`/dev/udp`-style redirection and the `dns` resolver module
//! that sits on top of this one need).

use crate::arch::nr;
use crate::arch::{from_ret, from_ret_i32, syscall2, syscall3, syscall4, syscall6, Errno};

/// Address family: IPv4.
pub const AF_INET: i32 = 2;
/// Address family: IPv6.
pub const AF_INET6: i32 = 10;

/// Socket type: reliable, connection-oriented byte stream (TCP).
pub const SOCK_STREAM: i32 = 1;
/// Socket type: connectionless, unreliable datagrams (UDP).
pub const SOCK_DGRAM: i32 = 2;
/// OR into `type` to atomically set close-on-exec on the new socket fd (a
/// Linux extension, same idea as [`crate::fd::O_CLOEXEC`]).
pub const SOCK_CLOEXEC: i32 = 0o2000000;
/// OR into `type` (or an `accept4` `flags` argument) to atomically set
/// non-blocking I/O on the new socket fd.
pub const SOCK_NONBLOCK: i32 = 0o0004000;

/// `shutdown(2)` mode: no further receives.
pub const SHUT_RD: i32 = 0;
/// `shutdown(2)` mode: no further sends.
pub const SHUT_WR: i32 = 1;
/// `shutdown(2)` mode: no further sends or receives.
pub const SHUT_RDWR: i32 = 2;

/// An IPv4 socket address (kernel `struct sockaddr_in`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SockAddrIn {
    family: u16,
    port: u16,     // network byte order
    addr: [u8; 4], // already in transmission (dotted-quad) order, no swap needed
    zero: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<SockAddrIn>() == 16);
const _: () = assert!(core::mem::offset_of!(SockAddrIn, port) == 2);
const _: () = assert!(core::mem::offset_of!(SockAddrIn, addr) == 4);

impl SockAddrIn {
    /// Build an IPv4 address from dotted-quad octets and a host-order port.
    pub fn new(octets: [u8; 4], port: u16) -> Self {
        SockAddrIn {
            family: AF_INET as u16,
            port: port.to_be(),
            addr: octets,
            zero: [0; 8],
        }
    }

    /// The address's dotted-quad octets.
    pub fn octets(&self) -> [u8; 4] {
        self.addr
    }

    /// The port, in host byte order.
    pub fn port(&self) -> u16 {
        u16::from_be(self.port)
    }
}

/// An IPv6 socket address (kernel `struct sockaddr_in6`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SockAddrIn6 {
    family: u16,
    port: u16,     // network byte order
    flowinfo: u32, // network byte order
    addr: [u8; 16],
    scope_id: u32,
}

const _: () = assert!(core::mem::size_of::<SockAddrIn6>() == 28);
const _: () = assert!(core::mem::offset_of!(SockAddrIn6, port) == 2);
const _: () = assert!(core::mem::offset_of!(SockAddrIn6, flowinfo) == 4);
const _: () = assert!(core::mem::offset_of!(SockAddrIn6, addr) == 8);
const _: () = assert!(core::mem::offset_of!(SockAddrIn6, scope_id) == 24);

impl SockAddrIn6 {
    /// Build an IPv6 address from 16 address octets and a host-order port.
    pub fn new(octets: [u8; 16], port: u16) -> Self {
        SockAddrIn6 {
            family: AF_INET6 as u16,
            port: port.to_be(),
            flowinfo: 0,
            addr: octets,
            scope_id: 0,
        }
    }

    /// The address's 16 octets.
    pub fn octets(&self) -> [u8; 16] {
        self.addr
    }

    /// The port, in host byte order.
    pub fn port(&self) -> u16 {
        u16::from_be(self.port)
    }
}

/// Create an endpoint for communication, returning the new socket fd.
/// `domain` is [`AF_INET`]/[`AF_INET6`]; `ty` is [`SOCK_STREAM`]/
/// [`SOCK_DGRAM`], optionally OR'd with [`SOCK_NONBLOCK`]/[`SOCK_CLOEXEC`];
/// `protocol` is `0` to let the kernel pick the default for `(domain, ty)`.
pub fn socket(domain: i32, ty: i32, protocol: i32) -> Result<i32, Errno> {
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall3(nr::SOCKET, domain as usize, ty as usize, protocol as usize) };
    from_ret_i32(ret)
}

/// Bind `fd` to a local IPv4 address.
pub fn bind_in(fd: i32, addr: &SockAddrIn) -> Result<(), Errno> {
    // SAFETY: `addr` is a valid, correctly-sized `sockaddr_in` the kernel
    // only reads.
    let ret = unsafe {
        syscall3(
            nr::BIND,
            fd as usize,
            addr as *const SockAddrIn as usize,
            core::mem::size_of::<SockAddrIn>(),
        )
    };
    from_ret(ret).map(|_| ())
}

/// Bind `fd` to a local IPv6 address.
pub fn bind_in6(fd: i32, addr: &SockAddrIn6) -> Result<(), Errno> {
    // SAFETY: `addr` is a valid, correctly-sized `sockaddr_in6` the kernel
    // only reads.
    let ret = unsafe {
        syscall3(
            nr::BIND,
            fd as usize,
            addr as *const SockAddrIn6 as usize,
            core::mem::size_of::<SockAddrIn6>(),
        )
    };
    from_ret(ret).map(|_| ())
}

/// Connect `fd` to a remote IPv4 address. Blocks until the connection is
/// established (or fails) unless `fd` was opened with [`SOCK_NONBLOCK`].
pub fn connect_in(fd: i32, addr: &SockAddrIn) -> Result<(), Errno> {
    // SAFETY: `addr` is a valid, correctly-sized `sockaddr_in` the kernel
    // only reads.
    let ret = unsafe {
        syscall3(
            nr::CONNECT,
            fd as usize,
            addr as *const SockAddrIn as usize,
            core::mem::size_of::<SockAddrIn>(),
        )
    };
    from_ret(ret).map(|_| ())
}

/// Connect `fd` to a remote IPv6 address. Blocks until the connection is
/// established (or fails) unless `fd` was opened with [`SOCK_NONBLOCK`].
pub fn connect_in6(fd: i32, addr: &SockAddrIn6) -> Result<(), Errno> {
    // SAFETY: `addr` is a valid, correctly-sized `sockaddr_in6` the kernel
    // only reads.
    let ret = unsafe {
        syscall3(
            nr::CONNECT,
            fd as usize,
            addr as *const SockAddrIn6 as usize,
            core::mem::size_of::<SockAddrIn6>(),
        )
    };
    from_ret(ret).map(|_| ())
}

/// Mark `fd` (a bound `SOCK_STREAM` socket) as accepting incoming
/// connections, with a backlog of up to `backlog` pending connections.
pub fn listen(fd: i32, backlog: i32) -> Result<(), Errno> {
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall2(nr::LISTEN, fd as usize, backlog as usize) };
    from_ret(ret).map(|_| ())
}

/// Accept a pending connection on the listening socket `fd`, returning the
/// new connected socket's fd. Discards the peer's address; use
/// [`accept4_in`]/[`accept4_in6`] to also learn who connected or to pass
/// `flags`.
pub fn accept(fd: i32) -> Result<i32, Errno> {
    // accept(fd, NULL, NULL).
    // SAFETY: null addr/addrlen is the documented way to discard the peer
    // address.
    let ret = unsafe { syscall3(nr::ACCEPT, fd as usize, 0, 0) };
    from_ret_i32(ret)
}

/// Accept a pending connection on an IPv4 listening socket `fd`, returning
/// the new connected socket's fd and the peer's address. `flags` may
/// include [`SOCK_NONBLOCK`]/[`SOCK_CLOEXEC`] for the new fd.
pub fn accept4_in(fd: i32, flags: i32) -> Result<(i32, SockAddrIn), Errno> {
    let mut addr = SockAddrIn {
        family: 0,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    let mut addrlen: u32 = core::mem::size_of::<SockAddrIn>() as u32;
    // SAFETY: `addr`/`addrlen` are valid, correctly-sized out-parameters the
    // kernel fills in.
    let ret = unsafe {
        syscall4(
            nr::ACCEPT4,
            fd as usize,
            &mut addr as *mut SockAddrIn as usize,
            &mut addrlen as *mut u32 as usize,
            flags as usize,
        )
    };
    let newfd = from_ret_i32(ret)?;
    Ok((newfd, addr))
}

/// Accept a pending connection on an IPv6 listening socket `fd`, returning
/// the new connected socket's fd and the peer's address. `flags` may
/// include [`SOCK_NONBLOCK`]/[`SOCK_CLOEXEC`] for the new fd.
pub fn accept4_in6(fd: i32, flags: i32) -> Result<(i32, SockAddrIn6), Errno> {
    let mut addr = SockAddrIn6 {
        family: 0,
        port: 0,
        flowinfo: 0,
        addr: [0; 16],
        scope_id: 0,
    };
    let mut addrlen: u32 = core::mem::size_of::<SockAddrIn6>() as u32;
    // SAFETY: `addr`/`addrlen` are valid, correctly-sized out-parameters the
    // kernel fills in.
    let ret = unsafe {
        syscall4(
            nr::ACCEPT4,
            fd as usize,
            &mut addr as *mut SockAddrIn6 as usize,
            &mut addrlen as *mut u32 as usize,
            flags as usize,
        )
    };
    let newfd = from_ret_i32(ret)?;
    Ok((newfd, addr))
}

/// Send `buf` on a connected socket `fd` (TCP, or a UDP socket that has
/// called `connect`), returning the number of bytes actually sent.
pub fn send(fd: i32, buf: &[u8], flags: i32) -> Result<usize, Errno> {
    // sendto(fd, buf, len, flags, NULL, 0).
    // SAFETY: `buf` is a valid slice of `buf.len()` bytes the kernel only
    // reads.
    let ret = unsafe {
        syscall6(
            nr::SENDTO,
            fd as usize,
            buf.as_ptr() as usize,
            buf.len(),
            flags as usize,
            0,
            0,
        )
    };
    from_ret(ret)
}

/// Receive into `buf` on a connected socket `fd`, returning the number of
/// bytes actually received (`0` means the peer closed the connection).
pub fn recv(fd: i32, buf: &mut [u8], flags: i32) -> Result<usize, Errno> {
    // recvfrom(fd, buf, len, flags, NULL, NULL).
    // SAFETY: `buf` is a valid, exclusively-borrowed slice of `buf.len()`
    // bytes the kernel writes at most that many of.
    let ret = unsafe {
        syscall6(
            nr::RECVFROM,
            fd as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
            flags as usize,
            0,
            0,
        )
    };
    from_ret(ret)
}

/// Send `buf` as a single UDP datagram to `dest` (an unconnected `fd` is
/// fine -- this is the usual way to use `SOCK_DGRAM`), returning the number
/// of bytes sent.
pub fn sendto_in(fd: i32, buf: &[u8], flags: i32, dest: &SockAddrIn) -> Result<usize, Errno> {
    // SAFETY: `buf` and `dest` are valid, correctly-sized inputs the kernel
    // only reads.
    let ret = unsafe {
        syscall6(
            nr::SENDTO,
            fd as usize,
            buf.as_ptr() as usize,
            buf.len(),
            flags as usize,
            dest as *const SockAddrIn as usize,
            core::mem::size_of::<SockAddrIn>(),
        )
    };
    from_ret(ret)
}

/// Send `buf` as a single UDP datagram to `dest` (IPv6).
pub fn sendto_in6(fd: i32, buf: &[u8], flags: i32, dest: &SockAddrIn6) -> Result<usize, Errno> {
    // SAFETY: `buf` and `dest` are valid, correctly-sized inputs the kernel
    // only reads.
    let ret = unsafe {
        syscall6(
            nr::SENDTO,
            fd as usize,
            buf.as_ptr() as usize,
            buf.len(),
            flags as usize,
            dest as *const SockAddrIn6 as usize,
            core::mem::size_of::<SockAddrIn6>(),
        )
    };
    from_ret(ret)
}

/// Receive a single UDP datagram into `buf`, returning the number of bytes
/// received and the sender's address.
pub fn recvfrom_in(fd: i32, buf: &mut [u8], flags: i32) -> Result<(usize, SockAddrIn), Errno> {
    let mut addr = SockAddrIn {
        family: 0,
        port: 0,
        addr: [0; 4],
        zero: [0; 8],
    };
    let mut addrlen: u32 = core::mem::size_of::<SockAddrIn>() as u32;
    // SAFETY: `buf` is a valid, exclusively-borrowed slice the kernel writes
    // at most `buf.len()` bytes of; `addr`/`addrlen` are valid,
    // correctly-sized out-parameters.
    let ret = unsafe {
        syscall6(
            nr::RECVFROM,
            fd as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
            flags as usize,
            &mut addr as *mut SockAddrIn as usize,
            &mut addrlen as *mut u32 as usize,
        )
    };
    let n = from_ret(ret)?;
    Ok((n, addr))
}

/// Receive a single UDP datagram into `buf` (IPv6), returning the number of
/// bytes received and the sender's address.
pub fn recvfrom_in6(fd: i32, buf: &mut [u8], flags: i32) -> Result<(usize, SockAddrIn6), Errno> {
    let mut addr = SockAddrIn6 {
        family: 0,
        port: 0,
        flowinfo: 0,
        addr: [0; 16],
        scope_id: 0,
    };
    let mut addrlen: u32 = core::mem::size_of::<SockAddrIn6>() as u32;
    // SAFETY: `buf` is a valid, exclusively-borrowed slice the kernel writes
    // at most `buf.len()` bytes of; `addr`/`addrlen` are valid,
    // correctly-sized out-parameters.
    let ret = unsafe {
        syscall6(
            nr::RECVFROM,
            fd as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
            flags as usize,
            &mut addr as *mut SockAddrIn6 as usize,
            &mut addrlen as *mut u32 as usize,
        )
    };
    let n = from_ret(ret)?;
    Ok((n, addr))
}

/// Shut down part or all of a full-duplex connection on `fd` without
/// closing the fd itself (`how` is [`SHUT_RD`]/[`SHUT_WR`]/[`SHUT_RDWR`]).
pub fn shutdown(fd: i32, how: i32) -> Result<(), Errno> {
    // SAFETY: plain integer arguments.
    let ret = unsafe { syscall2(nr::SHUTDOWN, fd as usize, how as usize) };
    from_ret(ret).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fd;

    const LOOPBACK: [u8; 4] = [127, 0, 0, 1];
    const LOOPBACK6: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

    #[test]
    fn sockaddr_in_roundtrips_octets_and_port() {
        let a = SockAddrIn::new([192, 168, 1, 42], 8080);
        assert_eq!(a.octets(), [192, 168, 1, 42]);
        assert_eq!(a.port(), 8080);
    }

    #[test]
    fn sockaddr_in6_roundtrips_octets_and_port() {
        let a = SockAddrIn6::new(LOOPBACK6, 53);
        assert_eq!(a.octets(), LOOPBACK6);
        assert_eq!(a.port(), 53);
    }

    #[test]
    fn tcp_loopback_echo_ipv4() {
        // Bind a listener on an ephemeral port (port 0), discover which port
        // the kernel picked isn't exposed without getsockname (out of scope
        // for this module), so bind to a fixed high port instead; retry a
        // couple of times in the unlikely case it's already in use.
        let mut last_err = None;
        for port in [58231u16, 58232, 58233] {
            let listener = socket(AF_INET, SOCK_STREAM, 0).expect("socket");
            if bind_in(listener, &SockAddrIn::new(LOOPBACK, port)).is_err() {
                fd::close(listener).ok();
                last_err = Some(port);
                continue;
            }
            listen(listener, 1).expect("listen");

            let client = socket(AF_INET, SOCK_STREAM, 0).expect("socket");
            connect_in(client, &SockAddrIn::new(LOOPBACK, port)).expect("connect");

            let (server_side, peer) = accept4_in(listener, 0).expect("accept4");
            assert_eq!(peer.octets(), LOOPBACK);

            send(client, b"ping", 0).expect("send");
            let mut buf = [0u8; 4];
            let n = recv(server_side, &mut buf, 0).expect("recv");
            assert_eq!(&buf[..n], b"ping");

            send(server_side, b"pong", 0).expect("send reply");
            let mut reply = [0u8; 4];
            let n = recv(client, &mut reply, 0).expect("recv reply");
            assert_eq!(&reply[..n], b"pong");

            shutdown(client, SHUT_RDWR).ok();
            fd::close(client).ok();
            fd::close(server_side).ok();
            fd::close(listener).ok();
            return;
        }
        panic!("all candidate ports were in use: {last_err:?}");
    }

    #[test]
    fn udp_loopback_datagram_ipv4() {
        let server = socket(AF_INET, SOCK_DGRAM, 0).expect("socket");
        bind_in(server, &SockAddrIn::new(LOOPBACK, 58234)).expect("bind");

        let client = socket(AF_INET, SOCK_DGRAM, 0).expect("socket");

        sendto_in(client, b"hello", 0, &SockAddrIn::new(LOOPBACK, 58234)).expect("sendto");

        let mut buf = [0u8; 5];
        let (n, from) = recvfrom_in(server, &mut buf, 0).expect("recvfrom");
        assert_eq!(&buf[..n], b"hello");
        assert_eq!(from.octets(), LOOPBACK);

        fd::close(client).ok();
        fd::close(server).ok();
    }

    #[test]
    fn connect_refused_when_nothing_listens() {
        let client = socket(AF_INET, SOCK_STREAM, 0).expect("socket");
        // Nothing should be listening on this port.
        let err = connect_in(client, &SockAddrIn::new(LOOPBACK, 58239)).unwrap_err();
        assert_eq!(err, Errno::ECONNREFUSED);
        fd::close(client).ok();
    }

    #[test]
    fn bad_domain_is_eafnosupport_or_einval() {
        let err = socket(999, SOCK_STREAM, 0).unwrap_err();
        // EAFNOSUPPORT (97) on Linux; accept EINVAL too in case of kernel
        // variance rather than hardcode a magic number this crate has no
        // named constant for.
        assert!(err == Errno(97) || err == Errno::EINVAL, "{err:?}");
    }

    #[test]
    fn accept_on_non_listening_socket_is_einval() {
        let s = socket(AF_INET, SOCK_STREAM, 0).expect("socket");
        assert_eq!(accept(s), Err(Errno::EINVAL));
        fd::close(s).ok();
    }
}
