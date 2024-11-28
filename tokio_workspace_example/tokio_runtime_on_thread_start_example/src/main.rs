use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

use tokio::{runtime::Builder, sync::Barrier};

fn main() {
  // println!("Hello, world!");
  let once = AtomicBool::new(true);
  let barrier = Arc::new(Barrier::new(2));

  let _runtime = Builder::new_multi_thread()
    .on_thread_start(|| {
      println!("thread started");
    })
    .on_thread_stop(|| {
      println!("thread stopping");
    })
    .on_thread_park({
      let barrier = barrier.clone();
      move || {
        let barrier = barrier.clone();
        if once.swap(false, Ordering::Relaxed) {
          tokio::spawn(async move {
            barrier.wait().await;
          });
        }
      }
    })
    .on_thread_unpark(|| {
      println!("thread unparking");
    })
    .build();
}
