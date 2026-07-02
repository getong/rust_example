use std::{net::Ipv4Addr, time::Duration};

use mainline::{Dht, MutableItem, Testnet};
use n0_error::{Result, StdResultExt};
use tokio::time::{Instant, sleep};

use crate::{
  identity::ClusterIdentity,
  options::{DhtOptions, KadServerOptions},
  records::{ClusterRecord, MemberRecord},
  util::{backoff_duration, now_unix_secs},
};

const DHT_VALUE_LIMIT: usize = 1000;
const MAX_DISCOVER_POLL_MS: u64 = 4000;
const DHT_RECORD_MAGIC: &[u8; 4] = b"IKD1";
const CLUSTER_RECORD_MAX_AGE_SECS: u64 = 24 * 60 * 60;

pub async fn run_kad_server(options: KadServerOptions) -> Result<()> {
  let testnet = Testnet::builder(options.nodes)
    .bind_address(options.bind)
    .build()
    .anyerr()?;

  let bootstrap = testnet.bootstrap.join(",");
  println!("mainline kad testnet is running");
  println!("export KAD_BOOTSTRAP={bootstrap}");
  println!("nodes:");
  for address in &testnet.bootstrap {
    println!("  {address}");
  }
  println!("keep this process running while servers and clients use the bootstrap list");

  tokio::signal::ctrl_c().await.anyerr()?;
  drop(testnet);
  Ok(())
}

pub(crate) fn build_dht(options: &DhtOptions) -> Result<Dht> {
  let mut builder = Dht::builder();

  builder
    .bind_address(options.bind)
    .request_timeout(options.request_timeout);

  if options.server_mode {
    builder.server_mode();
  }

  if let Some(port) = options.port {
    builder.port(port);
  }

  if !options.bootstrap.is_empty() {
    builder.bootstrap(&options.bootstrap);
  }

  builder.build().anyerr()
}

pub(crate) async fn publish_member(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  member: MemberRecord,
  max_members: usize,
) -> Result<()> {
  let mut errors = Vec::new();

  for attempt in 1..=3 {
    match publish_member_once(dht, cluster, member.clone(), max_members).await {
      Ok(()) => return Ok(()),
      Err(err) => {
        errors.push(err.to_string());
        if attempt < 3 {
          sleep(backoff_duration(125, attempt)).await;
        }
      }
    }
  }

  Err(n0_error::anyerr!(
    "failed to publish cluster member after retries: {}",
    errors.join("; ")
  ))
}

async fn publish_member_once(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  member: MemberRecord,
  max_members: usize,
) -> Result<()> {
  let public_key = cluster.public_key();
  let salt = Some(cluster.salt());
  let current = dht.get_mutable_most_recent(&public_key, salt).await;

  let mut record = current
    .as_ref()
    .map(|item| decode_cluster_record(item.value()))
    .transpose()?
    .unwrap_or_else(ClusterRecord::new);
  let cas = current.as_ref().map(MutableItem::seq);
  let seq = current.as_ref().map_or(1, |item| item.seq() + 1);

  record.insert_member(member, max_members);

  let value = encode_cluster_record_bounded(&mut record, DHT_VALUE_LIMIT)?;

  let item = MutableItem::new(cluster.signer(), &value, seq, salt);
  let outcome = dht.put_mutable(item, cas).await.anyerr()?;
  println!(
    "published cluster record seq={seq} target={} stored_at={}",
    outcome.target, outcome.stored_at
  );

  Ok(())
}

pub(crate) async fn discover_members(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  discover_timeout: Duration,
) -> Result<Vec<MemberRecord>> {
  let public_key = cluster.public_key();
  let salt = Some(cluster.salt());

  let deadline = Instant::now() + discover_timeout;
  let mut poll_attempt = 0;
  let mut newest_record: Option<ClusterRecord> = None;
  loop {
    if let Some(item) = dht.get_mutable_most_recent(&public_key, salt).await {
      let mut record = decode_cluster_record(item.value())?;
      if validate_cluster_record_freshness(&record).is_ok() {
        let is_newest = newest_record
          .as_ref()
          .is_none_or(|previous| record.is_newer_than(previous));
        if is_newest {
          newest_record = Some(record.clone());
          record
            .members
            .sort_by_key(|right| std::cmp::Reverse(right.updated_at));

          if !record.members.is_empty() {
            return Ok(record.members);
          }
        }
      }
    }

    if Instant::now() >= deadline {
      return Err(n0_error::anyerr!(
        "no cluster members found at target {}",
        cluster.target()
      ));
    }

    let delay =
      backoff_duration(100, poll_attempt).min(Duration::from_millis(MAX_DISCOVER_POLL_MS));
    sleep(delay).await;
    poll_attempt += 1;
  }
}

fn encode_cluster_record(record: &ClusterRecord) -> Result<Vec<u8>> {
  let encoded = postcard::to_stdvec(record).anyerr()?;
  let mut value = Vec::with_capacity(DHT_RECORD_MAGIC.len() + encoded.len());
  value.extend_from_slice(DHT_RECORD_MAGIC);
  value.extend_from_slice(&encoded);
  Ok(value)
}

fn encode_cluster_record_bounded(record: &mut ClusterRecord, limit: usize) -> Result<Vec<u8>> {
  if record.members.is_empty() {
    return encode_cluster_record(record);
  }

  if let Ok(value) = encode_cluster_record(record)
    && value.len() <= limit
  {
    return Ok(value);
  }

  let original_members = record.members.clone();
  let mut low = 1usize;
  let mut high = original_members.len();
  let mut best = None;

  while low <= high {
    let mid = low + (high - low) / 2;
    record.members = original_members[..mid].to_vec();

    match encode_cluster_record(record) {
      Ok(value) if value.len() <= limit => {
        best = Some((mid, value));
        low = mid + 1;
      }
      Ok(_) | Err(_) => {
        high = mid.saturating_sub(1);
      }
    }
  }

  if let Some((count, value)) = best {
    record.members = original_members[..count].to_vec();
    return Ok(value);
  }

  record.members = original_members[..1].to_vec();
  let value = encode_cluster_record(record)?;
  if value.len() > limit {
    return Err(n0_error::anyerr!(
      "cluster record is {} bytes, exceeding mainline BEP44 limit of {} bytes",
      value.len(),
      limit
    ));
  }
  Ok(value)
}

fn decode_cluster_record(value: &[u8]) -> Result<ClusterRecord> {
  if value.len() > DHT_VALUE_LIMIT {
    return Err(n0_error::anyerr!(
      "discovered cluster record exceeds size limit ({} > {} bytes)",
      value.len(),
      DHT_VALUE_LIMIT
    ));
  }

  if let Some(encoded) = value.strip_prefix(DHT_RECORD_MAGIC) {
    return postcard::from_bytes::<ClusterRecord>(encoded).anyerr();
  }

  serde_json::from_slice::<ClusterRecord>(value).anyerr()
}

fn validate_cluster_record_freshness(record: &ClusterRecord) -> Result<()> {
  let now = now_unix_secs();
  if record.updated_at > now.saturating_add(300) {
    return Err(n0_error::anyerr!(
      "discovered cluster record timestamp {} is too far in the future",
      record.updated_at
    ));
  }
  if now.saturating_sub(record.updated_at) > CLUSTER_RECORD_MAX_AGE_SECS {
    return Err(n0_error::anyerr!(
      "discovered cluster record timestamp {} is older than {} seconds",
      record.updated_at,
      CLUSTER_RECORD_MAX_AGE_SECS
    ));
  }
  Ok(())
}

pub(crate) fn local_dht_options(bootstrap: Vec<String>) -> DhtOptions {
  DhtOptions {
    server_mode: false,
    bind: Ipv4Addr::LOCALHOST,
    port: None,
    bootstrap,
    request_timeout: Duration::from_secs(2),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn member(index: usize, name_len: usize) -> MemberRecord {
    MemberRecord {
      endpoint_id: format!("{index:064x}"),
      name: "n".repeat(name_len),
      protocols: vec!["gossip".to_string()],
      blobs: Vec::new(),
      addrs: vec!["203.0.113.10:1234".to_string()],
      relay_urls: Vec::new(),
      updated_at: now_unix_secs().saturating_sub(index as u64),
    }
  }

  #[test]
  fn cluster_record_binary_round_trips() {
    let mut record = ClusterRecord::new();
    record.insert_member(member(1, 8), 16);

    let encoded = encode_cluster_record(&record).unwrap();
    assert!(encoded.starts_with(DHT_RECORD_MAGIC));

    let decoded = decode_cluster_record(&encoded).unwrap();
    assert_eq!(decoded.version, record.version);
    assert_eq!(decoded.updated_at, record.updated_at);
    assert_eq!(decoded.nonce, record.nonce);
    assert_eq!(decoded.members.len(), 1);
  }

  #[test]
  fn legacy_json_cluster_record_still_decodes() {
    let mut record = ClusterRecord::new();
    record.insert_member(member(1, 8), 16);
    let encoded = serde_json::to_vec(&record).unwrap();

    let decoded = decode_cluster_record(&encoded).unwrap();
    assert_eq!(decoded.members.len(), 1);
  }

  #[test]
  fn bounded_encoding_trims_without_exceeding_limit() {
    let mut record = ClusterRecord::new();
    for index in 0..32 {
      record.insert_member(member(index + 1, 160), 64);
    }

    let encoded = encode_cluster_record_bounded(&mut record, DHT_VALUE_LIMIT).unwrap();
    assert!(encoded.len() <= DHT_VALUE_LIMIT);
    assert!(!record.members.is_empty());
    assert!(record.members.len() < 32);
  }

  #[test]
  fn stale_cluster_record_is_rejected() {
    let mut record = ClusterRecord::new();
    record.updated_at = now_unix_secs().saturating_sub(CLUSTER_RECORD_MAX_AGE_SECS + 1);

    assert!(validate_cluster_record_freshness(&record).is_err());
  }
}
