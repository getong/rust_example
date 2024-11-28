use std::sync::Arc;

use tokio::{runtime::Runtime, sync::Notify};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Create the runtime
  let rt = Runtime::new()?;

  // Spawn the root task
  Ok(rt.block_on(async {
    let notify = Arc::new(Notify::new());
    let notify2 = notify.clone();

    tokio::spawn(async move {
      notify2.notified().await;
      println!("received notification");
    });

    println!("sending notification");
    notify.notify_one();
  }))
}
