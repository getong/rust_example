use futures::future::join_all;
use tokio::task;

#[tokio::main]
async fn main() {
  let mut handles = Vec::with_capacity(10_000);

  for i in 0..10_000 {
    let handle = task::spawn(async move {
      // Simulate some work
      println!("Task {} started", i);
    });
    handles.push(handle);
  }

  // Wait for all tasks to complete
  let results = join_all(handles).await;

  for result in results {
    match result {
      Ok(_) => println!("Task completed successfully"),
      Err(e) => eprintln!("Task failed: {:?}", e),
    }
  }
}
