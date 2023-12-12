use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct CallbackStore {
  cid: u8,
  cbs: Arc<Mutex<HashMap<u8, Box<dyn Fn(&str) + Send>>>>,
}

impl CallbackStore {
  pub fn new() -> CallbackStore {
    return CallbackStore {
      cid: 1,
      cbs: Arc::new(Mutex::new(HashMap::new())),
    };
  }

  pub fn add_callback(&mut self, cccb: Box<dyn Fn(&str) + Send>) {
    self.cid += 1;

    let cbs = self.cbs.clone();
    let mut cbs = cbs.try_lock().unwrap();
    cbs.insert(self.cid, cccb);
  }
}

pub fn dispatcher_loop(cstore: Arc<Mutex<HashMap<u8, Box<dyn Fn(&str) + Send>>>>) {
  thread::spawn(move || {
    loop {
      thread::sleep(Duration::from_millis(500));
      println!("Dispatching the callbacks...");

      let ccbs = cstore.clone();
      let cbs = ccbs.try_lock().unwrap();

      // Get the first callback
      let cb2 = cbs.get(&2).unwrap();
      cb2("hello world");

      // Get the first callback
      let cb3 = cbs.get(&3).unwrap();
      cb3("hello world!!!!!!!!!");

      // How many callbacks?
      println!("Currently: {} callbacks", cbs.len());
    }
  });
}

fn main() {
  println!("Storing some callbacks...");

  let mut store = CallbackStore::new();

  store.add_callback(Box::new(|msg| {
    println!("You got it! This is it: {}", msg);
  }));

  store.add_callback(Box::new(|msg| {
    println!("And this is: {}", msg);
  }));

  dispatcher_loop(store.cbs.clone());
  thread::sleep(Duration::from_millis(1000));

  loop {}
}
