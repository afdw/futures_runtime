use crate::executor::ActivityWakeHandle;
use crate::executor::Executor;
use crate::types::*;
use nix::errno::Errno;
use nix::unistd::close;
use nix::unistd::read;
use nix::unistd::write;
use std::collections::VecDeque;
use std::future::ready;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pub struct AsyncFile {
    fd: RawFd,
    read_buffer: VecDeque<u8>,
}

impl AsyncFile {
    pub fn from_fd(fd: RawFd) -> AsyncFile {
        AsyncFile {
            fd,
            read_buffer: VecDeque::new(),
        }
    }

    #[allow(unused)]
    pub fn fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for AsyncFile {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}

impl AsyncRead for AsyncFile {
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> Box<dyn Future<Output = BoxResult<usize>> + Unpin + 'a> {
        let fd = self.fd;
        match Executor::current().wake_on_activity(fd, Operation::READ) {
            Ok(activity_wake_handle) => Box::new(ReadFuture {
                async_file: self,
                buf,
                activity_wake_handle,
            }),
            Err(err) => Box::new(ready(Err(err))),
        }
    }

    fn take_buffer_back(&mut self, buf: &[u8]) {
        for &c in buf.iter().rev() {
            self.read_buffer.push_front(c);
        }
    }
}

struct ReadFuture<'a> {
    async_file: &'a mut AsyncFile,
    buf: &'a mut [u8],
    activity_wake_handle: ActivityWakeHandle,
}

impl<'a> Future for ReadFuture<'a> {
    type Output = BoxResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut buf = vec![0; BUF_SIZE];
            let result = read(self.async_file.fd, &mut buf[..]);
            match result {
                Ok(0) => return Poll::Ready(Ok(0)),
                Ok(read) => self.async_file.read_buffer.extend(&buf[0..read]),
                Err(nix::Error::Sys(Errno::EAGAIN)) => {
                    self.activity_wake_handle.set_waker(cx.waker());
                    break;
                }
                Err(err) => return Poll::Ready(Err(Box::new(err))),
            }
        }
        match self.async_file.read_buffer.len() {
            0 => Poll::Pending,
            _ => {
                let output = usize::min(self.async_file.read_buffer.len(), self.buf.len());
                let drain = self
                    .async_file
                    .read_buffer
                    .drain(0..output)
                    .collect::<Vec<_>>();
                if let Err(err) = self.buf.write_all(&drain) {
                    return Poll::Ready(Err(Box::new(err)));
                }
                Poll::Ready(Ok(output))
            }
        }
    }
}

impl AsyncWrite for AsyncFile {
    fn write<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Box<dyn Future<Output = BoxResult<usize>> + Unpin + 'a> {
        let fd = self.fd;
        match Executor::current().wake_on_activity(fd, Operation::WRITE) {
            Ok(activity_wake_handle) => Box::new(WriteFuture {
                async_file: self,
                buf,
                activity_wake_handle,
            }),
            Err(err) => Box::new(ready(Err(err))),
        }
    }
}

struct WriteFuture<'a> {
    async_file: &'a mut AsyncFile,
    buf: &'a [u8],
    activity_wake_handle: ActivityWakeHandle,
}

impl<'a> Future for WriteFuture<'a> {
    type Output = BoxResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = write(self.async_file.fd, &self.buf[..]);
        match result {
            Ok(written) => Poll::Ready(Ok(written)),
            Err(nix::Error::Sys(Errno::EAGAIN)) => {
                self.activity_wake_handle.set_waker(cx.waker());
                Poll::Pending
            }
            Err(err) => Poll::Ready(Err(Box::new(err))),
        }
    }
}
