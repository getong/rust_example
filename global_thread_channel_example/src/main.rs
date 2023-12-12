use lazy_static::lazy_static;
use std::sync::mpsc::sync_channel;

pub mod lib {
  pub struct Bar(*mut i32);
  impl Bar {
    pub fn new() -> Self {
      Bar(Box::into_raw(Box::new(0)))
    }
    pub fn set(&mut self, v: i32) {
      unsafe { *self.0 = v };
    }
    pub fn get(&self) -> i32 {
      unsafe { *self.0 }
    }
  }
}

enum Message {
  Set(i32),
  Get,
  Shutdown,
}

enum Reply {
  Set,
  Get(i32),
  Shutdown,
}

fn global_thread(
  receiver: std::sync::mpsc::Receiver<(Message, std::sync::mpsc::SyncSender<Reply>)>,
) {
  // Start the global state
  let mut bar = lib::Bar::new();

  // Handle messages
  loop {
    let (mesg, reply_channel) = receiver.recv().unwrap();
    match mesg {
      Message::Set(v) => {
        eprintln!("    worker: setting value to {}", v);
        bar.set(v);
        reply_channel.send(Reply::Set).unwrap();
      }
      Message::Get => {
        let v = bar.get();
        eprintln!("    worker: getting value = {}", v);
        reply_channel.send(Reply::Get(v)).unwrap();
      }
      Message::Shutdown => {
        eprintln!("    worker: shutting down");
        reply_channel.send(Reply::Shutdown).unwrap();
        break;
      }
    }
  }
}

// This can be cloned happily
// and supports Send+Sync
struct GlobalProxy {
  channel: std::sync::mpsc::SyncSender<(Message, std::sync::mpsc::SyncSender<Reply>)>,
}

impl GlobalProxy {
  pub fn set(&self, v: i32) {
    eprintln!("  proxy: setting value to {}", v);
    let (a, b) = sync_channel(0);
    self.channel.send((Message::Set(v), a)).unwrap();
    let m = b.recv().unwrap();
    assert!(matches!(m, Reply::Set));
  }

  pub fn get(&self) -> i32 {
    eprintln!("  proxy: getting value");
    let (a, b) = sync_channel(0);
    self.channel.send((Message::Get, a)).unwrap();
    let m = b.recv().unwrap();
    if let Reply::Get(v) = m {
      eprintln!("  proxy: got value={}", v);
      v
    } else {
      unreachable!();
    }
  }

  pub fn die(&self) {
    eprintln!("Telling worker thread to shut down");
    let (a, b) = sync_channel(0);
    self.channel.send((Message::Shutdown, a)).unwrap();
    let m = b.recv().unwrap();
    assert!(matches!(m, Reply::Shutdown));
  }
}

lazy_static! {
    static ref G: GlobalProxy = {
        // Create com channels
        let (to_global, from_world) = sync_channel(0);
        // Keep one end for the proxy,
        let global = GlobalProxy{ channel: to_global};
        // The other goes to the worker thread
        std::thread::spawn(|| {global_thread(from_world)});
        global
    };
}

pub fn main() {
  eprintln!("global.get() = {}", G.get());
  eprintln!("global.set(10)",);
  G.set(10);
  eprintln!("global.get() = {}", G.get());

  G.die()
}
