use std::{fs, time::Duration};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
  native_token::LAMPORTS_PER_SOL,
  signature::{Signature, Signer},
  signer::keypair::Keypair,
  transaction::{Transaction, VersionedTransaction},
};
use solana_system_interface::instruction as system_instruction;
use tokio::{sync::oneshot, time::sleep};

use crate::{queue::Priority, rpc_pool::RpcPool};

#[derive(Debug, Clone)]
pub struct SendRequest {
  pub request_id: String,
  pub tx_b64: String,
  pub priority: Priority,
}

#[derive(Debug)]
pub struct SendResponse {
  pub request_id: String,
  pub signature: String,
  pub worker_id: usize,
  pub elapsed_ms: u128,
  pub priority: Priority,
}

pub struct SendTask {
  pub req: SendRequest,
  pub resp: oneshot::Sender<Result<SendResponse, String>>,
}

#[derive(Debug)]
pub struct ConfirmRequest {
  pub request_id: String,
  pub signature: Signature,
  pub priority: Priority,
}

#[derive(Debug)]
pub struct ConfirmResponse {
  pub request_id: String,
  pub signature: String,
  pub confirmed: bool,
  pub reason: Option<String>,
  pub worker_id: usize,
  pub attempts: usize,
  pub elapsed_ms: u128,
  pub priority: Priority,
}

pub struct ConfirmTask {
  pub req: ConfirmRequest,
  pub resp: oneshot::Sender<ConfirmResponse>,
}

pub fn decode_transaction(tx_b64: &str) -> Result<VersionedTransaction, String> {
  let bytes = general_purpose::STANDARD
    .decode(tx_b64)
    .map_err(|e| format!("base64 decode error: {e}"))?;
  bincode::deserialize::<VersionedTransaction>(&bytes)
    .map_err(|e| format!("bincode decode error: {e}"))
}

pub fn load_requests_from_file(path: &str) -> Result<Vec<SendRequest>> {
  let content = fs::read_to_string(path).with_context(|| format!("read TX_FILE {path}"))?;
  let mut requests = Vec::new();

  for (idx, line) in content.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    let mut parts = line.split(',').map(str::trim);
    let request_id = parts
      .next()
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing request_id", idx + 1))?;
    let second = parts
      .next()
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing base64 tx", idx + 1))?;
    let third = parts.next().filter(|s| !s.is_empty());

    let (priority, tx_b64) = if let Some(tx_b64) = third {
      let priority = Priority::parse(second)
        .ok_or_else(|| anyhow!("line {} invalid priority {}", idx + 1, second))?;
      (priority, tx_b64)
    } else {
      (Priority::Normal, second)
    };

    if parts.next().is_some() {
      return Err(anyhow!("line {} has too many fields", idx + 1));
    }

    requests.push(SendRequest {
      request_id: request_id.to_string(),
      tx_b64: tx_b64.to_string(),
      priority,
    });
  }

  if requests.is_empty() {
    return Err(anyhow!("TX_FILE has no valid requests"));
  }

  Ok(requests)
}

pub async fn build_demo_requests(pool: &RpcPool, count: usize) -> Result<Vec<SendRequest>> {
  let payer = Keypair::new();
  let recipient = Keypair::new();
  let client = pool
    .get()
    .await
    .map_err(|err| anyhow!("rpc pool error: {err}"))?;

  let airdrop_sig = client
    .request_airdrop(&payer.pubkey(), LAMPORTS_PER_SOL)
    .await
    .context("airdrop failed (try a local validator or set TX_FILE)")?;

  let mut attempts = 0;
  let mut delay = Duration::from_millis(200);
  loop {
    attempts += 1;
    let confirmed = client
      .confirm_transaction_with_commitment(&airdrop_sig, CommitmentConfig::confirmed())
      .await
      .context("airdrop confirmation failed")?;
    if confirmed.value {
      break;
    }
    if attempts >= 15 {
      return Err(anyhow!("airdrop not confirmed after {attempts} attempts"));
    }
    sleep(delay).await;
    let next_ms = delay.as_millis().saturating_mul(2) as u64;
    delay = Duration::from_millis(next_ms.min(2_000));
  }

  let blockhash = client
    .get_latest_blockhash()
    .await
    .context("get_latest_blockhash failed")?;

  let mut requests = Vec::with_capacity(count);
  for idx in 0 .. count {
    let lamports = 1_000 + idx as u64;
    let ix = system_instruction::transfer(&payer.pubkey(), &recipient.pubkey(), lamports);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], blockhash);
    let versioned = VersionedTransaction::from(tx);
    let bytes = bincode::serialize(&versioned).context("bincode encode failed")?;
    let tx_b64 = general_purpose::STANDARD.encode(bytes);

    requests.push(SendRequest {
      request_id: format!("demo-{idx}"),
      tx_b64,
      priority: Priority::Normal,
    });
  }

  Ok(requests)
}
