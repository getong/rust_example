use futures::future::try_join_all;
use std::fmt;
use tokio::io::AsyncBufReadExt;

#[derive(Debug)]
pub enum MyError {
  Other,
}

impl std::error::Error for MyError {}

impl fmt::Display for MyError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::Other => write!(f, "Some other error occured!"),
    }
  }
}

async fn do_some_work(task_name: &str) -> Result<(), MyError> {
  println!("This is task {}, doing some work", task_name);
  let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
  let mut buffer = Vec::new();

  let _fut = reader.read_until(b'\n', &mut buffer).await;
  println!("Input was: {:?}", buffer);
  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), MyError> {
  let results = try_join_all(vec![
    do_some_work("Task 1"),
    do_some_work("Task 2"),
    do_some_work("Task 3"),
  ])
  .await;
  results?;

  Ok(())
}
