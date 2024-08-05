use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
  // println!("Hello, world!");
  let shared_data = Arc::new((Mutex::new(false), Condvar::new()));
  let shared_data_clone = Arc::clone(&shared_data);
  let stop = Arc::new(AtomicBool::new(false));
  let stop_clone = Arc::clone(&stop);

  let _background_thread = thread::spawn(move || {
    let (lock, cvar) = &*shared_data_clone;
    let mut received_value = lock.lock().unwrap();
    while !stop.load(Relaxed) {
      received_value = cvar.wait(received_value).unwrap();
      println!("Received value: {}", *received_value);
    }
  });

  let updater_thread = thread::spawn(move || {
    let (lock, cvar) = &*shared_data;
    let values = [false, true, false, true];

    for i in 0 .. 4 {
      let update_value = values[i as usize];
      println!("Updating value to {}...", update_value);
      *lock.lock().unwrap() = update_value;
      cvar.notify_one();
      thread::sleep(Duration::from_nanos(1));
    }
    stop_clone.store(true, Relaxed);
    println!("STOP has been updated");
    cvar.notify_one();
  });
  updater_thread.join().unwrap();
}
