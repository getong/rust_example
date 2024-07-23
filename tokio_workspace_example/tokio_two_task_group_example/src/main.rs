use tokio::sync::oneshot;
use tokio::task;
use tokio::time::{sleep, Duration};

async fn task_1(mut stop_rx: oneshot::Receiver<()>) {
  tokio::select! {
    _ = async {
      loop {
        println!("Task 1 is working...");
        sleep(Duration::from_secs(1)).await;
      }
    } => {},
    _ = &mut stop_rx => {
      println!("Task 1 received stop signal.");
    }
  }
}

async fn task_2(mut stop_rx: oneshot::Receiver<()>) {
  tokio::select! {
    _ = async {
      loop {
        println!("Task 2 is working...");
        sleep(Duration::from_secs(1)).await;
      }
    } => {},
    _ = &mut stop_rx => {
      println!("Task 2 received stop signal.");
    }
  }
}

#[tokio::main]
async fn main() {
  let (stop_tx1, stop_rx1) = oneshot::channel();
  let (stop_tx2, stop_rx2) = oneshot::channel();

  let task1_handle = task::spawn(task_1(stop_rx1));
  let task2_handle = task::spawn(task_2(stop_rx2));

  // Simulate some work
  sleep(Duration::from_secs(5)).await;

  // Send the stop signal
  let _ = stop_tx1.send(());
  let _ = stop_tx2.send(());

  // Wait for both tasks to finish
  let _ = task1_handle.await;
  let _ = task2_handle.await;

  println!("Both tasks have stopped.");
}
