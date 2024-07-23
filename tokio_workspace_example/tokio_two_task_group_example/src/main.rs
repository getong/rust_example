use rand::Rng;
use tokio::sync::oneshot;
use tokio::task;
use tokio::time::{sleep, Duration};

async fn task_1(mut stop_rx: oneshot::Receiver<()>, send_tx: oneshot::Sender<()>) {
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

  _ = send_tx.send(());
}

async fn task_2(mut stop_rx: oneshot::Receiver<()>, send_tx: oneshot::Sender<()>) {
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

  _ = send_tx.send(());
}

#[tokio::main]
async fn main() {
  let (stop_tx1, stop_rx1) = oneshot::channel();
  let (stop_tx2, stop_rx2) = oneshot::channel();

  let task1_handle = task::spawn(task_1(stop_rx1, stop_tx2));
  let task2_handle = task::spawn(task_2(stop_rx2, stop_tx1));

  // Simulate some work
  sleep(Duration::from_secs(3)).await;

  let mut rng = rand::thread_rng();
  match rng.gen_range(1u64..3u64) {
    1 => {
      println!("task 1 stop");
      task1_handle.abort();
    }

    _ => {
      println!("task 2 stop");
      task2_handle.abort();
    }
  }

  // Wait for both tasks to finish
  let _ = task1_handle.await;
  let _ = task2_handle.await;

  sleep(Duration::from_secs(3)).await;
  println!("Both tasks have stopped.");
}
