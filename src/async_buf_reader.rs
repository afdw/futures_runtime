use crate::types::*;
use std::collections::VecDeque;

pub struct AsyncBufReader<'a, I>
where
    I: AsyncRead,
{
    inner: &'a mut I,
    buf: VecDeque<u8>,
}

impl<'a, I> AsyncBufReader<'a, I>
where
    I: AsyncRead,
{
    pub fn new(inner: &'a mut I) -> AsyncBufReader<'a, I> {
        AsyncBufReader {
            inner,
            buf: VecDeque::new(),
        }
    }

    pub async fn read_line(&mut self) -> BoxResult<Option<String>> {
        while self.buf.iter().position(|&c| c == b'\n').is_none() {
            let mut buf = vec![0; BUF_SIZE];
            let read = self.inner.read(&mut buf[..]).await?;
            if read == 0 {
                return Ok(None);
            }
            self.buf.extend(&buf[0..read]);
        }
        let len = self.buf.iter().position(|&c| c == b'\n').unwrap() + 1;
        Ok(Some(
            String::from_utf8(self.buf.drain(0..len).take(len - 1).collect())?.replace('\r', ""),
        ))
    }
}

impl<'a, I> Drop for AsyncBufReader<'a, I>
where
    I: AsyncRead,
{
    fn drop(&mut self) {
        self.inner
            .take_buffer_back(&self.buf.drain(..).collect::<Vec<_>>());
    }
}
