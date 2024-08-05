use std::{thread, time::Duration};

mod sync_primitives {
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::{Arc, Condvar, Mutex};

  struct Wg {
    counter: AtomicUsize,
    mu: Mutex<bool>,
    condvar: Condvar,
  }

  impl Wg {
    fn new() -> Self {
      Wg {
        counter: AtomicUsize::new(0),
        mu: Mutex::new(false),
        condvar: Condvar::new(),
      }
    }
  }

  pub struct WaitGroup(Arc<Wg>);

  impl WaitGroup {
    pub fn new() -> Self {
      WaitGroup(Arc::new(Wg::new()))
    }

    pub fn wait(&self) {
      let mut started = self.0.mu.lock().unwrap();
      while !*started {
        started = self.0.condvar.wait(started).unwrap();
        if self.0.counter.load(Ordering::Relaxed) == 0 {
          *started = true;
        }
      }
    }
  }

  impl Clone for WaitGroup {
    fn clone(&self) -> Self {
      self.0.counter.fetch_add(1, Ordering::Relaxed);
      Self(self.0.clone())
    }
  }

  impl Drop for WaitGroup {
    fn drop(&mut self) {
      self.0.counter.fetch_sub(1, Ordering::Relaxed);
      self.0.condvar.notify_one();
    }
  }
}

fn main() {
  use sync_primitives::WaitGroup;

  let wg = WaitGroup::new();

  for i in 1 .. 5 {
    let wg = wg.clone();
    thread::spawn(move || {
      println!("t{} going to sleep for {} secs", i, i);
      thread::sleep(Duration::from_secs(i));
      println!("t{} woke up", i);

      drop(wg);
    });
  }

  wg.wait();

  println!("exiting..");
}
