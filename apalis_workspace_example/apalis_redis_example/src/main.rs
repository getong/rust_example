use apalis::prelude::*;
use apalis_redis::RedisStorage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Email {
  to: String,
}

async fn send_email(task: Email) -> Result<(), BoxDynError> {
  println!("Sending email to {}", task.to);
  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), BoxDynError> {
  let redis_url =
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned());
  let conn = apalis_redis::connect(redis_url).await?;
  let mut storage = RedisStorage::new(conn);

  let task = Email {
    to: "test@example.com".to_owned(),
  };

  storage.push(task).await?;

  let worker = WorkerBuilder::new("tasty-pear")
    .backend(storage)
    .build(send_email);

  worker.run().await?;

  Ok(())
}
