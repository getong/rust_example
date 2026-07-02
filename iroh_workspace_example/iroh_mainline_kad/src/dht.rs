use std::{net::Ipv4Addr, time::Duration};

use mainline::{Dht, MutableItem, Testnet};
use n0_error::{Result, StdResultExt};
use tokio::time::{Instant, sleep};

use crate::{
  identity::ClusterIdentity,
  options::{DhtOptions, KadServerOptions},
  records::{ClusterRecord, MemberRecord},
  util::backoff_duration,
};

const DHT_VALUE_LIMIT: usize = 1000;
const MAX_DISCOVER_POLL_MS: u64 = 4000;

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

  let mut record = match current.as_ref() {
    Some(item) => serde_json::from_slice::<ClusterRecord>(item.value()).anyerr()?,
    None => ClusterRecord::new(),
  };
  let cas = current.as_ref().map(MutableItem::seq);
  let seq = current.as_ref().map_or(1, |item| item.seq() + 1);

  record.insert_member(member, max_members);

  let mut value = serde_json::to_vec(&record).anyerr()?;
  while value.len() > DHT_VALUE_LIMIT && record.members.len() > 1 {
    record.members.pop();
    value = serde_json::to_vec(&record).anyerr()?;
  }

  if value.len() > DHT_VALUE_LIMIT {
    return Err(n0_error::anyerr!(
      "cluster record is {} bytes, exceeding mainline BEP44 limit of {} bytes",
      value.len(),
      DHT_VALUE_LIMIT
    ));
  }

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
  loop {
    if let Some(item) = dht.get_mutable_most_recent(&public_key, salt).await {
      let value = item.value();
      if value.len() > DHT_VALUE_LIMIT {
        return Err(n0_error::anyerr!(
          "discovered cluster record exceeds size limit ({} > {} bytes)",
          value.len(),
          DHT_VALUE_LIMIT
        ));
      }
      let mut record = serde_json::from_slice::<ClusterRecord>(value).anyerr()?;
      record
        .members
        .sort_by_key(|right| std::cmp::Reverse(right.updated_at));

      if !record.members.is_empty() {
        return Ok(record.members);
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

pub(crate) fn local_dht_options(bootstrap: Vec<String>) -> DhtOptions {
  DhtOptions {
    server_mode: false,
    bind: Ipv4Addr::LOCALHOST,
    port: None,
    bootstrap,
    request_timeout: Duration::from_secs(2),
  }
}
