use tokio::sync::mpsc::{self, Sender};
const BATCH_SIZE: usize = 10;
const TOTAL_SIZE: usize = 10000;
use parking_lot::RwLock;
use std::sync::Arc;

#[tokio::main]
async fn main() {
  let mut total_size: usize = 0;

  let mut current_size: usize = BATCH_SIZE;
  let (tx, mut rx) = mpsc::channel(BATCH_SIZE);

  let data = Arc::new(RwLock::new(5));

  for i in 0 .. BATCH_SIZE {
    let tx = tx.clone();
    let data = data.clone();
    spawn_task(tx, i, data).await;
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
        let data = data.clone();
        spawn_task(tx, current_size, data).await;
      }
    }
  }

  let data = data.read();
  println!("data is {:?}", *data);
}

async fn perform_task(tx: Sender<usize>, id: usize, data: Arc<RwLock<usize>>) {
  println!("Task {} is running", id);
  if id % 2 == 0 {
    let mut data = data.write();
    *data = *data + 1;
  } else {
    let data = data.read();
    println!("data is {:?}", *data);
  }
  // Simulate some async work
  // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
  _ = tx.try_send(id);
}

async fn spawn_task(tx: Sender<usize>, id: usize, data: Arc<RwLock<usize>>) {
  tokio::spawn(async move {
    perform_task(tx, id, data).await;
  });
}
