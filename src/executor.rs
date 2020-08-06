use crate::epoll::Epoll;
use crate::epoll::EpollEntryId;
use crate::types::*;
use send_wrapper::SendWrapper;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::time::Duration;
use std::time::Instant;

thread_local! {
    static EXECUTOR: RefCell<Option<Executor>> = RefCell::new(None);
    static WAKE_CALLED: Cell<bool> = Cell::new(false);
}

pub type TimerId = u64;

pub struct ActivityWakeHandle {
    id: TimerId,
}

impl ActivityWakeHandle {
    pub fn set_waker(&self, waker: &Waker) {
        let waker = waker.clone();
        Executor::current()
            .inner
            .borrow_mut()
            .epoll
            .modify(self.id, move || {
                waker.wake_by_ref();
            });
    }
}

impl Drop for ActivityWakeHandle {
    fn drop(&mut self) {
        Executor::current()
            .inner
            .borrow_mut()
            .epoll
            .remove(self.id)
            .unwrap();
    }
}

pub struct TimeoutWakeHandle {
    id: EpollEntryId,
}

impl TimeoutWakeHandle {
    pub fn set_waker(&self, waker: &Waker) {
        Executor::current()
            .inner
            .borrow_mut()
            .timers
            .get_mut(&self.id)
            .unwrap()
            .1
            .replace(waker.clone());
    }
}

impl Drop for TimeoutWakeHandle {
    fn drop(&mut self) {
        Executor::current()
            .inner
            .borrow_mut()
            .timers
            .remove(&self.id);
    }
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

struct TaskWaker {
    task: SendWrapper<RefCell<Option<Task>>>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        if let Some(task) = self.task.borrow_mut().take() {
            Executor::current()
                .inner
                .borrow()
                .queue
                .borrow_mut()
                .push_back(task);
        } else {
            WAKE_CALLED.with(|wake_called| wake_called.replace(true));
        }
    }
}

struct ExecutorInner {
    epoll: Epoll,
    queue: RefCell<VecDeque<Task>>,
    last_timer_id: TimerId,
    timers: HashMap<TimerId, (Instant, Option<Waker>)>,
}

#[derive(Clone)]
pub struct Executor {
    inner: Rc<RefCell<ExecutorInner>>,
}

impl Executor {
    pub fn new() -> BoxResult<Executor> {
        Ok(Executor {
            inner: Rc::new(RefCell::new(ExecutorInner {
                epoll: Epoll::new()?,
                queue: RefCell::new(VecDeque::new()),
                last_timer_id: 0,
                timers: HashMap::new(),
            })),
        })
    }

    pub fn current() -> Executor {
        EXECUTOR.with(|executor| executor.borrow().clone().unwrap())
    }

    pub fn wake_on_activity(
        &self,
        fd: RawFd,
        operation: Operation,
    ) -> BoxResult<ActivityWakeHandle> {
        Ok(ActivityWakeHandle {
            id: Executor::current()
                .inner
                .borrow_mut()
                .epoll
                .add(fd, operation, || {})?,
        })
    }

    pub fn wake_at_time(&self, time: Instant) -> TimeoutWakeHandle {
        let id = Executor::current().inner.borrow().last_timer_id;
        Executor::current().inner.borrow_mut().last_timer_id += 1;
        Executor::current()
            .inner
            .borrow_mut()
            .timers
            .insert(id, (time, None));
        TimeoutWakeHandle { id }
    }

    pub fn spawn<F>(&self, future: F) -> BoxResult<()>
    where
        F: Future<Output = ()> + 'static,
    {
        self.inner.borrow().queue.borrow_mut().push_back(Task {
            future: Box::pin(future),
        });
        Ok(())
    }

    pub fn run(&self) -> BoxResult<()> {
        loop {
            while let Some(mut task) =
                drop_temporaries!(self.inner.borrow().queue.borrow_mut().pop_front())
            {
                let task_waker = Arc::new(TaskWaker {
                    task: SendWrapper::new(RefCell::new(None)),
                });
                EXECUTOR.with(|executor| executor.borrow_mut().replace(self.clone()));
                WAKE_CALLED.with(|wake_called| wake_called.replace(false));
                if let Poll::Pending = task
                    .future
                    .as_mut()
                    .poll(&mut Context::from_waker(&task_waker.clone().into()))
                {
                    if WAKE_CALLED.with(|wake_called| wake_called.take()) {
                        self.inner.borrow().queue.borrow_mut().push_back(task);
                    } else {
                        task_waker.task.borrow_mut().replace(task);
                    }
                }
            }
            let timeout = if self.inner.borrow().timers.is_empty() {
                None
            } else {
                Some(
                    self.inner
                        .borrow()
                        .timers
                        .values()
                        .map(|(time, _)| time.checked_duration_since(Instant::now()))
                        .min()
                        .flatten()
                        .unwrap_or_else(Duration::zero),
                )
            };
            self.inner.borrow().epoll.wait(timeout)?;
            let mut to_remove = Vec::new();
            for (id, (time, waker)) in &self.inner.borrow().timers {
                if Instant::now() >= *time {
                    if let Some(waker) = waker {
                        waker.wake_by_ref();
                    }
                    to_remove.push(*id);
                }
            }
            for id in to_remove {
                self.inner.borrow_mut().timers.remove(&id);
            }
        }
    }
}
