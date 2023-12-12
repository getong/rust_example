use chrono::Local;
use tokio::{self, runtime::Runtime, time};

fn now() -> String {
  Local::now().format("%F %T").to_string()
}

async fn do_one() {
  println!("doing one: {}", now());
  time::sleep(time::Duration::from_secs(2)).await;
  println!("do one done: {}", now());
}

async fn do_two() {
  println!("doing two: {}", now());
  time::sleep(time::Duration::from_secs(1)).await;
  println!("do two done: {}", now());
}

fn main() {
  let rt = Runtime::new().unwrap();
  rt.block_on(async {
    tokio::join!(do_one(), do_two()); // 等待两个任务均完成，才继续向下执行代码
    println!("all done: {}", now());
  });
}
