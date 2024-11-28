use std::sync::Arc;

use tokio::{sync::Semaphore, task};

async fn perform_task(id: usize) {
  println!("Task {} is running", id);
  // Simulate some async work
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[tokio::main]
async fn main() {
  let max_concurrent_tasks = 10;
  let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));
  let mut handles = Vec::with_capacity(10_000);

  for i in 0 .. 10_000 {
    let permit = semaphore.clone().acquire_owned().await.unwrap();
    let handle = task::spawn(async move {
      perform_task(i).await;
      drop(permit); // Release the permit when the task is done
    });

    handles.push(handle);

    // Wait for all current batch tasks to finish before spawning more
    if i % max_concurrent_tasks == 0 && i != 0 {
      for handle in handles.drain(..) {
        handle.await.unwrap();
      }
    }
  }

  // Await remaining tasks
  for handle in handles {
    handle.await.unwrap();
  }

  println!("All tasks completed.");
}
