pub use std::os::unix::io::RawFd;

use nix::fcntl::fcntl;
use nix::fcntl::FcntlArg;
use std::future::Future;
use std::io;
use std::io::ErrorKind;
use std::pin::Pin;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

pub const BUF_SIZE: usize = 1024;

pub enum Operation {
    READ,
    WRITE,
}

pub fn dup_fd(fd: RawFd) -> BoxResult<RawFd> {
    Ok(fcntl(fd, FcntlArg::F_DUPFD_CLOEXEC(0))?)
}

pub trait AsyncRead {
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> Box<dyn Future<Output = BoxResult<usize>> + Unpin + 'a>;

    fn take_buffer_back(&mut self, buf: &[u8]);
}

pub trait AsyncWrite {
    fn write<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Box<dyn Future<Output = BoxResult<usize>> + Unpin + 'a>;

    fn write_all<'a>(
        &'a mut self,
        mut buf: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = BoxResult<()>> + 'a>> {
        Box::pin(async move {
            while !buf.is_empty() {
                match self.write(buf).await {
                    Ok(0) => {
                        return Err(Box::new(io::Error::new(
                            ErrorKind::WriteZero,
                            "failed to write whole buffer",
                        )) as Box<dyn std::error::Error>);
                    }
                    Ok(n) => buf = &buf[n..],
                    Err(err) => return Err(err),
                }
            }
            Ok(())
        })
    }
}

#[macro_export]
macro_rules! async_write {
    ($dst:expr, $($arg:tt)*) => ($dst.write_all(format!($($arg)*).as_bytes()))
}

#[macro_export]
macro_rules! async_writeln {
    ($dst:expr) => (
        async_write!($dst, "\n")
    );
    ($dst:expr,) => (
        async_writeln!($dst)
    );
    ($dst:expr, $($arg:tt)*) => (
        $dst.write_all((format!($($arg)*) + "\n").as_bytes())
    );
}

#[macro_export]
macro_rules! drop_temporaries {
    ($e:expr) => {{
        let x = $e;
        x
    }};
}
