use std::{io, os::unix::io::RawFd, sync::mpsc::Sender};

use crate::{
  poll::{Poll, Registry},
  EventId,
};

pub struct Reactor {
  pub registry: Option<Registry>,
}

impl Reactor {
  pub fn new() -> Self {
    Self { registry: None }
  }

  pub fn run(&mut self, sender: Sender<EventId>) {
    let poller = Poll::new();
    let registry = poller.get_registry();

    self.registry = Some(registry);

    std::thread::spawn(move || {
      let mut events: Vec<libc::epoll_event> = Vec::with_capacity(1024);

      loop {
        poller.poll(&mut events);

        for e in &events {
          sender.send(e.u64 as EventId).expect("channel works");
        }
      }
    });
  }

  pub fn read_interest(&mut self, fd: RawFd, event_id: EventId) -> io::Result<()> {
    self
      .registry
      .as_mut()
      .expect("registry is set")
      .register_read(fd, event_id)
  }

  pub fn write_interest(&mut self, fd: RawFd, event_id: EventId) -> io::Result<()> {
    self
      .registry
      .as_mut()
      .expect("registry is set")
      .register_write(fd, event_id)
  }

  pub fn close(&mut self, fd: RawFd) -> io::Result<()> {
    self
      .registry
      .as_mut()
      .expect("registry is set")
      .remove_interests(fd)
  }
}
