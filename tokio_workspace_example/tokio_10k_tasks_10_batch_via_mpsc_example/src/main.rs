use tokio::sync::mpsc::{self, Sender};
const BATCH_SIZE: usize = 10;
const TOTAL_SIZE: usize = 10000;

#[tokio::main]
async fn main() {
  let mut total_size: usize = 0;

  let mut current_size: usize = BATCH_SIZE;
  let (tx, mut rx) = mpsc::channel(BATCH_SIZE);

  for i in 0 .. BATCH_SIZE {
    let tx = tx.clone();
    spawn_task(tx, i).await;
  }

  while let Some(num) = rx.recv().await {
    println!("recv num is {:?}", num);
    total_size = total_size + 1;

    if total_size >= TOTAL_SIZE {
      break;
    } else {
      let tx = tx.clone();
      current_size = current_size + 1;
      if current_size <= TOTAL_SIZE {
        spawn_task(tx, current_size).await;
      }
    }
  }
}

async fn perform_task(tx: Sender<usize>, id: usize) {
  println!("Task {} is running", id);
  // Simulate some async work
  // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
  _ = tx.try_send(id);
}

async fn spawn_task(tx: Sender<usize>, id: usize) {
  tokio::spawn(async move {
    perform_task(tx, id).await;
  });
}
