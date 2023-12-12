use crate::EventId;
use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::io::RawFd;

const READ_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLIN;
const WRITE_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLOUT;

macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

pub struct Poll {
  epoll_fd: RawFd,
}

impl Poll {
  pub fn new() -> Self {
    let epoll_fd = syscall!(epoll_create1(0)).expect("can create epoll");
    if let Ok(flags) = syscall!(fcntl(epoll_fd, libc::F_GETFD)) {
      let _ = syscall!(fcntl(epoll_fd, libc::F_SETFD, flags | libc::FD_CLOEXEC));
    }
    Self { epoll_fd }
  }

  pub fn get_registry(&self) -> Registry {
    Registry::new(self.epoll_fd)
  }

  pub fn poll(&self, events: &mut Vec<libc::epoll_event>) {
    events.clear();
    let res = match syscall!(epoll_wait(
      self.epoll_fd,
      events.as_mut_ptr() as *mut libc::epoll_event,
      1024,
      1000 as libc::c_int,
    )) {
      Ok(v) => v,
      Err(e) => panic!("error during epoll wait: {}", e),
    };

    // safe  as long as the kernel does nothing wrong - copied from mio
    unsafe { events.set_len(res as usize) };
  }
}

pub fn read_event(event_id: EventId) -> libc::epoll_event {
  libc::epoll_event {
    events: READ_FLAGS as u32,
    u64: event_id as u64,
  }
}

pub fn write_event(event_id: EventId) -> libc::epoll_event {
  libc::epoll_event {
    events: WRITE_FLAGS as u32,
    u64: event_id as u64,
  }
}

pub fn close(fd: RawFd) {
  let _ = syscall!(close(fd));
}

pub struct Registry {
  epoll_fd: RawFd,
  io_sources: HashMap<RawFd, HashSet<Interest>>,
}

impl Registry {
  pub fn new(epoll_fd: RawFd) -> Self {
    Self {
      epoll_fd,
      io_sources: HashMap::new(),
    }
  }

  pub fn register_read(&mut self, fd: RawFd, event_id: EventId) -> io::Result<()> {
    let interests = self.io_sources.entry(fd).or_insert(HashSet::new());

    if interests.is_empty() {
      syscall!(epoll_ctl(
        self.epoll_fd,
        libc::EPOLL_CTL_ADD,
        fd,
        &mut read_event(event_id)
      ))?;
    } else {
      syscall!(epoll_ctl(
        self.epoll_fd,
        libc::EPOLL_CTL_MOD,
        fd,
        &mut read_event(event_id)
      ))?;
    }

    interests.clear();
    interests.insert(Interest::READ);

    Ok(())
  }

  pub fn register_write(&mut self, fd: RawFd, event_id: EventId) -> io::Result<()> {
    let interests = self.io_sources.entry(fd).or_insert(HashSet::new());

    if interests.is_empty() {
      syscall!(epoll_ctl(
        self.epoll_fd,
        libc::EPOLL_CTL_ADD,
        fd,
        &mut write_event(event_id)
      ))?;
    } else {
      syscall!(epoll_ctl(
        self.epoll_fd,
        libc::EPOLL_CTL_MOD,
        fd,
        &mut write_event(event_id)
      ))?;
    }

    interests.clear();
    interests.insert(Interest::WRITE);

    Ok(())
  }

  pub fn remove_interests(&mut self, fd: RawFd) -> io::Result<()> {
    self.io_sources.remove(&fd);
    syscall!(epoll_ctl(
      self.epoll_fd,
      libc::EPOLL_CTL_DEL,
      fd,
      std::ptr::null_mut()
    ))?;
    close(fd);

    Ok(())
  }
}

#[derive(PartialEq, Hash, Eq)]
pub enum Interest {
  READ,
  WRITE,
}
