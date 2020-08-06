use crate::types::*;
use nix::sys::epoll::epoll_create1;
use nix::sys::epoll::epoll_ctl;
use nix::sys::epoll::epoll_wait;
use nix::sys::epoll::EpollCreateFlags;
use nix::sys::epoll::EpollEvent;
use nix::sys::epoll::EpollFlags;
use nix::sys::epoll::EpollOp;
use nix::unistd::close;
use std::collections::HashMap;
use std::time::Duration;

pub type EpollEntryId = u64;

const EPOLL_EVENTS_LIMIT: usize = 100;

struct EpollEntryData {
    fd: RawFd,
    callback: Box<dyn Fn()>,
}

pub struct Epoll {
    fd: RawFd,
    last_id: EpollEntryId,
    handles: HashMap<EpollEntryId, EpollEntryData>,
}

impl Epoll {
    pub fn new() -> BoxResult<Epoll> {
        Ok(Epoll {
            fd: epoll_create1(EpollCreateFlags::EPOLL_CLOEXEC)?,
            last_id: 0,
            handles: HashMap::new(),
        })
    }

    #[allow(unused)]
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn add(
        &mut self,
        fd: RawFd,
        operation: Operation,
        callback: impl Fn() + 'static,
    ) -> BoxResult<EpollEntryId> {
        let fd = dup_fd(fd)?;
        let id = self.last_id;
        self.last_id += 1;
        self.handles.insert(
            id,
            EpollEntryData {
                fd,
                callback: Box::new(callback),
            },
        );
        epoll_ctl(
            self.fd,
            EpollOp::EpollCtlAdd,
            fd,
            &mut EpollEvent::new(
                EpollFlags::EPOLLET
                    | match operation {
                        Operation::READ => EpollFlags::EPOLLIN,
                        Operation::WRITE => EpollFlags::EPOLLOUT,
                    },
                id,
            ),
        )?;
        Ok(id)
    }

    pub fn modify(&mut self, id: EpollEntryId, callback: impl Fn() + 'static) {
        self.handles.get_mut(&id).unwrap().callback = Box::new(callback);
    }

    pub fn remove(&mut self, id: EpollEntryId) -> BoxResult<()> {
        epoll_ctl(self.fd, EpollOp::EpollCtlDel, self.handles[&id].fd, None)?;
        close(self.handles[&id].fd)?;
        self.handles.remove(&id);
        Ok(())
    }

    pub fn wait(&self, timeout: Option<Duration>) -> BoxResult<()> {
        let mut epoll_events = vec![EpollEvent::empty(); EPOLL_EVENTS_LIMIT];
        let epoll_event_count = epoll_wait(
            self.fd,
            &mut epoll_events[..],
            match timeout {
                Some(timeout) => timeout.as_millis() as isize,
                None => -1,
            },
        );
        for epoll_event in epoll_events.into_iter().take(epoll_event_count?) {
            (self.handles[&epoll_event.data()].callback)();
        }
        Ok(())
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        for id in self.handles.keys().copied().collect::<Vec<_>>() {
            self.remove(id).unwrap();
        }
        close(self.fd).unwrap();
    }
}
