// copy from https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html

use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
  let mut set = JoinSet::new();

  for i in 0..10 {
    set.spawn(async move { i });
  }

  let mut seen = [false; 10];
  while let Some(res) = set.join_next().await {
    let idx = res.unwrap();
    seen[idx] = true;
  }

  for i in 0..10 {
    assert!(seen[i]);
  }
}
