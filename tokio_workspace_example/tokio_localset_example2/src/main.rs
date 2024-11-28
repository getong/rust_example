use chrono::Local;
use tokio::{runtime::Runtime, task::LocalSet, time};

fn now() -> String {
  Local::now().format("%F %T").to_string()
}

fn main() {
  let rt = Runtime::new().unwrap();
  let local_tasks = LocalSet::new();

  rt.block_on(async {
    local_tasks
      .run_until(async move {
        println!("run until function begin");
        time::sleep(time::Duration::from_secs(2)).await;
        println!("run until function end");
      })
      .await;
  });

  // 向本地任务队列中添加新的异步任务，但现在不会执行
  local_tasks.spawn_local(async {
    println!("local task1");
    time::sleep(time::Duration::from_secs(3)).await;
    println!("local task1 done");
  });

  local_tasks.spawn_local(async {
    println!("local task2");
    time::sleep(time::Duration::from_secs(3)).await;
    println!("local task2 done");
  });

  println!("before local tasks running: {}", now());
  rt.block_on(async {
    // 开始执行本地任务队列中的所有异步任务，并等待它们全部完成
    local_tasks.await;
  });
}
