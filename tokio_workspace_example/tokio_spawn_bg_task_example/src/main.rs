// copy from https://rustcc.cn/article?id=ba4f86c6-667d-4acb-89a1-e2fb0617f524
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> std::io::Result<()> {
  let now = Instant::now();

  let mut handles = Vec::with_capacity(10);
  for i in 0..10 {
    handles.push(tokio::spawn(my_bg_task(i)));
  }

  // Do something time-consuming while the background tasks execute.
  std::thread::sleep(Duration::from_millis(100));
  println!("Finished time-consuming task.");

  // Wait for all of them to complete.
  for handle in handles {
    handle.await?;
  }

  println!("总耗时：{} ms", now.elapsed().as_millis());

  // -------
  let now = Instant::now();

  let mut handles = Vec::with_capacity(10);
  for i in 0..10 {
    handles.push(my_bg_task(i)); // 没有把 Future 变成任务
  }

  // std::thread::sleep(Duration::from_millis(120));
  println!("Finished time-consuming task.");

  futures::future::join_all(handles).await; // 但是 join_all 会等待所有 Future 并发执行完
  println!("总耗时：{} ms", now.elapsed().as_millis());
  Ok(())
}

async fn my_bg_task(i: u64) {
  let millis = 100;
  println!("Task {} sleeping for {} ms.", i, millis);
  sleep(Duration::from_millis(millis)).await;
  println!("Task {} stopping.", i);
}
