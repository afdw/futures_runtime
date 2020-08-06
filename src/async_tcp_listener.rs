use crate::executor::ActivityWakeHandle;
use crate::executor::Executor;
use crate::listen_socket::ListenSocket;
use crate::types::*;
use nix::errno::Errno;
use std::collections::VecDeque;
use std::future::ready;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pub struct AsyncTcpListener {
    listen_socket: ListenSocket,
    queue: VecDeque<RawFd>,
}

impl AsyncTcpListener {
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> BoxResult<AsyncTcpListener> {
        Ok(AsyncTcpListener {
            listen_socket: ListenSocket::bind(addr)?,
            queue: VecDeque::new(),
        })
    }

    pub fn incoming(&mut self) -> Box<dyn Future<Output = BoxResult<RawFd>> + Unpin + '_> {
        let fd = self.listen_socket.fd();
        match Executor::current().wake_on_activity(fd, Operation::READ) {
            Ok(activity_wake_handle) => Box::new(SocketListenFuture {
                async_tcp_listener: self,
                activity_wake_handle,
            }),
            Err(err) => Box::new(ready(Err(err))),
        }
    }
}

struct SocketListenFuture<'a> {
    async_tcp_listener: &'a mut AsyncTcpListener,
    activity_wake_handle: ActivityWakeHandle,
}

impl<'a> Future for SocketListenFuture<'a> {
    type Output = BoxResult<RawFd>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.async_tcp_listener.listen_socket.accept() {
                Ok(socket) => self.async_tcp_listener.queue.push_back(socket),
                Err(nix::Error::Sys(Errno::EAGAIN)) => {
                    self.activity_wake_handle.set_waker(cx.waker());
                    break;
                }
                Err(err) => return Poll::Ready(Err(Box::new(err))),
            }
        }
        match self.async_tcp_listener.queue.pop_front() {
            None => Poll::Pending,
            Some(fd) => Poll::Ready(Ok(fd)),
        }
    }
}
