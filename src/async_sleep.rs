use crate::executor::Executor;
use crate::executor::TimeoutWakeHandle;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

pub fn async_sleep(timeout: Duration) -> impl Future<Output = ()> {
    let time = Instant::now() + timeout;
    let timeout_wake_handle = Executor::current().wake_at_time(time);
    AsyncSleepFuture {
        time,
        timeout_wake_handle,
    }
}

struct AsyncSleepFuture {
    time: Instant,
    timeout_wake_handle: TimeoutWakeHandle,
}

impl Future for AsyncSleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.time {
            Poll::Ready(())
        } else {
            self.timeout_wake_handle.set_waker(cx.waker());
            Poll::Pending
        }
    }
}
