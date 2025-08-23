mod distributed_lock;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use distributed_lock::{LockGuard, RedisDistributedLock};
use tokio::task::JoinSet;

async fn simulate_client(client_id: u32, nodes: Vec<String>) -> Result<()> {
  println!("[Client {}] Starting...", client_id);

  let lock = RedisDistributedLock::new(
    nodes,
    "shared_resource_lock".to_string(),
    Duration::from_secs(10),
  )
  .await?;

  println!("[Client {}] Attempting to acquire lock...", client_id);

  if lock
    .acquire_with_retry(5, Duration::from_millis(500))
    .await?
  {
    println!(
      "[Client {}] Lock acquired! Starting critical section work...",
      client_id
    );

    if let Ok(Some(info)) = lock.get_lock_info().await {
      println!("[Client {}] Lock info: {:?}", client_id, info);
      println!(
        "[Client {}] Remaining TTL: {:?}",
        client_id,
        info.remaining_time()
      );
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("[Client {}] Work completed.", client_id);

    println!(
      "[Client {}] Extending lock for 5 more seconds...",
      client_id
    );
    if lock.extend(Duration::from_secs(5)).await? {
      println!("[Client {}] Lock extended successfully", client_id);
      tokio::time::sleep(Duration::from_secs(1)).await;
    }

    println!("[Client {}] Releasing lock...", client_id);
    if lock.release().await? {
      println!("[Client {}] Lock released successfully", client_id);
    } else {
      println!(
        "[Client {}] Lock was already released or expired",
        client_id
      );
    }
  } else {
    println!(
      "[Client {}] Failed to acquire lock after retries",
      client_id
    );
  }

  Ok(())
}

async fn demonstrate_lock_guard(nodes: Vec<String>) -> Result<()> {
  println!("\n=== Demonstrating LockGuard with auto-release ===");

  let lock = RedisDistributedLock::new(
    nodes,
    "auto_release_lock".to_string(),
    Duration::from_secs(30),
  )
  .await?;

  if lock.acquire().await? {
    let _guard = LockGuard::new(lock.clone(), true);
    println!("Lock acquired with guard");

    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("Work completed, guard will auto-release on drop");
  }

  tokio::time::sleep(Duration::from_millis(500)).await;

  if !lock.is_locked().await? {
    println!("Lock was successfully auto-released");
  }

  Ok(())
}

async fn demonstrate_lock_contention(nodes: Vec<String>) -> Result<()> {
  println!("\n=== Demonstrating lock contention with multiple clients ===");

  let mut tasks = JoinSet::new();

  for client_id in 1 ..= 5 {
    let nodes_clone = nodes.clone();
    tasks.spawn(async move {
      if let Err(e) = simulate_client(client_id, nodes_clone).await {
        eprintln!("[Client {}] Error: {}", client_id, e);
      }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  while let Some(result) = tasks.join_next().await {
    if let Err(e) = result {
      eprintln!("Task failed: {}", e);
    }
  }

  Ok(())
}

async fn demonstrate_concurrent_workers(nodes: Vec<String>) -> Result<()> {
  println!("\n=== Demonstrating concurrent workers with shared lock ===");

  let shared_nodes = Arc::new(nodes);
  let mut workers = JoinSet::new();

  for worker_id in 1 ..= 3 {
    let nodes = Arc::clone(&shared_nodes);

    workers.spawn(async move {
      for task_id in 1 ..= 2 {
        let lock = RedisDistributedLock::new(
          nodes.to_vec(),
          format!("task_lock_{}", task_id % 2),
          Duration::from_secs(5),
        )
        .await
        .unwrap();

        println!("[Worker {}] Attempting task {}", worker_id, task_id);

        if lock
          .acquire_with_retry(3, Duration::from_millis(200))
          .await
          .unwrap()
        {
          println!("[Worker {}] Processing task {}", worker_id, task_id);
          tokio::time::sleep(Duration::from_millis(500)).await;

          lock.release().await.unwrap();
          println!("[Worker {}] Completed task {}", worker_id, task_id);
        } else {
          println!(
            "[Worker {}] Skipped task {} (couldn't acquire lock)",
            worker_id, task_id
          );
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
      }
    });
  }

  while let Some(_) = workers.join_next().await {}

  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("Redis Cluster Distributed Lock Example");
  println!("======================================\n");

  let redis_nodes = vec![
    "redis://127.0.0.1:7000".to_string(),
    "redis://127.0.0.1:7001".to_string(),
    "redis://127.0.0.1:7002".to_string(),
    "redis://127.0.0.1:7003".to_string(),
    "redis://127.0.0.1:7004".to_string(),
    "redis://127.0.0.1:7005".to_string(),
  ];

  println!("Connecting to Redis cluster nodes:");
  for node in &redis_nodes {
    println!("  - {}", node);
  }
  println!();

  demonstrate_lock_contention(redis_nodes.clone()).await?;

  demonstrate_lock_guard(redis_nodes.clone()).await?;

  demonstrate_concurrent_workers(redis_nodes.clone()).await?;

  println!("\n=== All demonstrations completed ===");

  Ok(())
}
