use std::time::Duration;
use tokio::{task, time}; // 1.3.0

#[tokio::main]
async fn main() {
  let forever = task::spawn(async {
    let mut interval = time::interval(Duration::from_millis(10));

    loop {
      interval.tick().await;
      do_something().await;
    }
  });

  let _ = forever.await;
}

async fn do_something() {
  eprintln!("do_something");
}
