mod config;
mod queue;
mod requests;
mod rpc_pool;
mod worker;

use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, Result};
use solana_sdk::signature::Signature;
use tokio::task::JoinSet;

use crate::{
  config::Config,
  queue::ShardedQueue,
  requests::{build_demo_requests, load_requests_from_file, ConfirmRequest, ConfirmTask, SendTask},
  rpc_pool::{build_rpc_pool, EndpointSelector},
  worker::{ConfirmPool, SendPool},
};

#[tokio::main]
async fn main() -> Result<()> {
  let cfg = Config::from_env();
  let send_workers = cfg.send_workers.max(1);
  let confirm_workers = cfg.confirm_workers.max(1);
  let endpoints = Arc::new(EndpointSelector::new(cfg.rpc_urls.clone()));

  println!(
    "rpc_urls=[{}], send_workers={}, confirm_workers={}, queue_size={}, rpc_pool_size={}",
    endpoints.list().join(","),
    send_workers,
    confirm_workers,
    cfg.queue_size,
    cfg.rpc_pool_size
  );

  let max_len = cfg.queue_size.max(1);
  let send_queue = Arc::new(ShardedQueue::<SendTask>::new(send_workers, max_len));
  let confirm_queue = Arc::new(ShardedQueue::<ConfirmTask>::new(confirm_workers, max_len));

  let rpc_pool = build_rpc_pool(&cfg, endpoints.clone())?;
  let send_pool = Arc::new(SendPool::new(
    send_workers,
    &cfg,
    send_queue,
    rpc_pool.clone(),
    endpoints.clone(),
  ));
  let confirm_pool = Arc::new(ConfirmPool::new(
    confirm_workers,
    &cfg,
    confirm_queue,
    rpc_pool.clone(),
    endpoints.clone(),
  ));

  let requests = if let Some(path) = &cfg.tx_file {
    load_requests_from_file(path)?
  } else {
    build_demo_requests(&rpc_pool, cfg.demo_count).await?
  };

  let mut join_set = JoinSet::new();
  for req in requests {
    let send_pool = Arc::clone(&send_pool);
    let confirm_pool = Arc::clone(&confirm_pool);

    join_set.spawn(async move {
      let send_res = send_pool.submit(req).await;
      match send_res {
        Ok(sent) => {
          println!(
            "sent request_id={}, signature={}, worker={}, elapsed_ms={}, priority={}",
            sent.request_id, sent.signature, sent.worker_id, sent.elapsed_ms, sent.priority
          );
          let signature = Signature::from_str(&sent.signature)
            .map_err(|e| anyhow!("invalid signature {}: {e}", sent.signature))?;
          let confirm_req = ConfirmRequest {
            request_id: sent.request_id.clone(),
            signature,
            priority: sent.priority,
          };
          let confirm_res = confirm_pool
            .submit(confirm_req)
            .await
            .map_err(|e| anyhow!("confirm error for {}: {e}", sent.request_id))?;

          Ok::<_, anyhow::Error>(confirm_res)
        }
        Err(err) => Err(anyhow!("send error: {err}")),
      }
    });
  }

  while let Some(result) = join_set.join_next().await {
    match result {
      Ok(Ok(confirm)) => {
        if confirm.confirmed {
          println!(
            "confirmed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}, \
             priority={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms,
            confirm.priority
          );
        } else {
          println!(
            "failed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}, \
             reason={}, priority={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms,
            confirm.reason.unwrap_or_else(|| "unknown".to_string()),
            confirm.priority
          );
        }
      }
      Ok(Err(err)) => {
        eprintln!("task error: {err}");
      }
      Err(join_err) => {
        eprintln!("join error: {join_err}");
      }
    }
  }

  Ok(())
}
