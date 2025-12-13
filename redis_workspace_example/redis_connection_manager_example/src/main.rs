use std::env;

use redis::{cluster::ClusterClient, cluster_async::ClusterConnection, RedisError, RedisResult};

#[tokio::main]
async fn main() -> RedisResult<()> {
  let password = env::var("VALKEY_PASSWORD").unwrap_or_else(|_| "abc123".to_string());
  let nodes: Vec<String> = env::var("VALKEY_NODES")
    .ok()
    .filter(|s| !s.trim().is_empty())
    .map(|s| {
      s.split(',')
        .map(|item| item.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
    })
    .unwrap_or_else(|| {
      vec![7001, 7002, 7003]
        .into_iter()
        .map(|port| format!("redis://:{password}@127.0.0.1:{port}/"))
        .collect()
    });

  if nodes.is_empty() {
    panic!("VALKEY_NODES is empty; provide at least one node uri");
  }

  println!("Connecting to Valkey cluster nodes: {:?}", nodes);

  let cluster_client = ClusterClient::new(nodes)?;
  let mut conn = connect_with_retry(&cluster_client, 5, 1_000).await?;
  wait_for_cluster_ready(&mut conn, 5, 1_000).await?;

  for i in 1 .. 100 {
    let key = format!("abc_{}", i);
    println!("the key is {}", key);
    redis::pipe()
      .atomic()
      .set(&key, 1u8)
      .expire(&key, 60)
      .query_async::<()>(&mut conn)
      .await?;

    println!("key {} set ok", key);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
  }

  Ok(())
}

async fn connect_with_retry(
  client: &ClusterClient,
  max_attempts: usize,
  delay_ms: u64,
) -> Result<ClusterConnection, RedisError> {
  let mut last_err: Option<RedisError> = None;

  for attempt in 1..=max_attempts {
    match client.get_async_connection().await {
      Ok(conn) => return Ok(conn),
      Err(err) => {
        last_err = Some(err);
        if attempt < max_attempts {
          eprintln!(
            "Valkey cluster connect attempt {} failed; retrying in {} ms...",
            attempt, delay_ms
          );
          tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
      }
    }
  }

  Err(last_err.unwrap_or_else(|| {
    RedisError::from((redis::ErrorKind::IoError, "No connections found"))
  }))
}

async fn wait_for_cluster_ready(
  conn: &mut ClusterConnection,
  max_attempts: usize,
  delay_ms: u64,
) -> RedisResult<()> {
  for attempt in 1..=max_attempts {
    let info: RedisResult<String> = redis::cmd("CLUSTER").arg("INFO").query_async(conn).await;
    match info {
      Ok(text) if text.contains("cluster_state:ok") => {
        println!("Valkey cluster is ready (cluster_state:ok).");
        return Ok(());
      }
      Ok(text) => {
        eprintln!(
          "Cluster not ready yet (attempt {}): {}. Retrying in {} ms...",
          attempt,
          text.trim().replace('\n', " "),
          delay_ms
        );
      }
      Err(err) => {
        eprintln!(
          "Cluster INFO failed (attempt {}): {}. Retrying in {} ms...",
          attempt, err, delay_ms
        );
      }
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
  }

  Err(RedisError::from((
    redis::ErrorKind::IoError,
    "Cluster not ready after retries",
  )))
}
