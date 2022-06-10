use async_executor::Executor;
use futures_lite::future;

fn main() {
    // println!("Hello, world!");

    // Create a new executor.
    let ex = Executor::new();

    // Spawn a task.
    let task = ex.spawn(async {
        println!("Hello world");
    });

    // Run the executor until the task completes.
    future::block_on(ex.run(task));
}
