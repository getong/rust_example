use chrono::Local;
use std::thread;
use tokio::{self, runtime::Runtime, time};

fn now() -> String {
  Local::now().format("%F %T").to_string()
}

fn main() {
  let rt = Runtime::new().unwrap();

  // 进入runtime，但不阻塞当前线程
  let guard1 = rt.enter();

  // 生成的异步任务将放入当前的runtime上下文中执行
  tokio::spawn(async {
    time::sleep(time::Duration::from_secs(5)).await;
    println!("task1 sleep over: {}", now());
  });

  // 释放runtime上下文，这并不会删除runtime
  drop(guard1);

  // 可以再次进入runtime
  let guard2 = rt.enter();
  tokio::spawn(async {
    time::sleep(time::Duration::from_secs(4)).await;
    println!("task2 sleep over: {}", now());
  });

  drop(guard2);

  // 阻塞当前线程，等待异步任务的完成
  thread::sleep(std::time::Duration::from_secs(10));
}
