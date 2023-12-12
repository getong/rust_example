use tokio::runtime::Runtime;
// use tokio::task;

fn main() {
  // Create the first Tokio runtime
  let runtime1 = Runtime::new().unwrap();

  // Spawn a task on the first runtime
  runtime1.spawn(async {
    println!("Task 1 executed on runtime 1");
  });

  // Create the second Tokio runtime
  let runtime2 = Runtime::new().unwrap();

  // Spawn a task on the second runtime
  runtime2.spawn(async {
    println!("Task 2 executed on runtime 2");
  });

  // Block the main thread until both runtimes complete their tasks
  runtime1.block_on(async {});
  runtime2.block_on(async {});
}
// copy from chatgpt
