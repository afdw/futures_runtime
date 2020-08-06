use crate::types::*;
use nix::sys::socket::accept4;
use nix::sys::socket::bind;
use nix::sys::socket::listen;
use nix::sys::socket::setsockopt;
use nix::sys::socket::socket;
use nix::sys::socket::sockopt;
use nix::sys::socket::AddressFamily;
use nix::sys::socket::InetAddr;
use nix::sys::socket::SockAddr;
use nix::sys::socket::SockFlag;
use nix::sys::socket::SockType;
use nix::unistd::close;
use std::net::SocketAddr;

pub struct ListenSocket {
    fd: RawFd,
}

impl ListenSocket {
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> BoxResult<ListenSocket> {
        let listen_socket = ListenSocket {
            fd: socket(
                AddressFamily::Inet,
                SockType::Stream,
                SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
                None,
            )?,
        };
        setsockopt(listen_socket.fd, sockopt::ReuseAddr, &true)?;
        bind(
            listen_socket.fd,
            &SockAddr::Inet(InetAddr::from_std(&addr.into())),
        )?;
        listen(listen_socket.fd, usize::max_value())?;
        Ok(listen_socket)
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn accept(&mut self) -> nix::Result<RawFd> {
        accept4(self.fd, SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC)
    }
}

impl Clone for ListenSocket {
    fn clone(&self) -> Self {
        ListenSocket {
            fd: dup_fd(self.fd).unwrap(),
        }
    }
}

impl Drop for ListenSocket {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}
