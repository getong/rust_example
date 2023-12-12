use chrono::Local;
use tokio::runtime::Runtime;

fn main() {
  let rt = Runtime::new().unwrap();
  rt.block_on(async {
    println!("before sleep: {}", Local::now().format("%F %T.%3f"));
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    println!("after sleep: {}", Local::now().format("%F %T.%3f"));
  });
}
