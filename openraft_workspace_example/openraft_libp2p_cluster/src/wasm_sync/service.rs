//! The per-node wasm sync service: announce what we have, download what
//! peers have and we lack.
//!
//! One loop per process, spawned for control and worker nodes alike:
//!
//!   - every [`WASM_SYNC_ANNOUNCE_INTERVAL`] (and immediately after installing a new module) it
//!     publishes the local module inventory on gossipsub;
//!   - inventory announcements from peers (fed in by the swarm loop through
//!     [`super::notify_announcement`]) maintain a provider map `sha256 -> nodes that have it`;
//!   - modules announced by peers but absent from the local store are downloaded chunk by chunk,
//!     rotating providers per chunk so every holder shares the upload load; failed chunks retry on
//!     the other providers, and a round that cannot finish (all providers down) is resumed by a
//!     later round from the persisted partial state.
//!
//! Downloads run inside the service loop, one module at a time — wasm module
//! distribution is a background concern that must never compete with raft
//! traffic for connection bandwidth in bursts.

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use libp2p::PeerId;

use super::{
  WasmInventoryAnnouncement, WasmModuleManifest, WasmModuleSummary, WasmSyncRequest,
  WasmSyncResponse, downloader::PartialDownload, is_valid_module_name, local_inventory,
};
use crate::{
  NodeId,
  network::{swarm::WASM_MODULES_TOPIC, transport::Libp2pNetworkFactory},
  signal::ShutdownRx,
  tasks::wasm_runtime::WasmModuleStore,
};

/// Default inventory announce cadence (the
/// [`crate::runtime_config::RuntimeConfig::wasm_sync_announce_interval_secs`]
/// default). Also the effective retry cadence for downloads that stalled
/// with every provider unreachable: each announce round from a returning
/// provider re-triggers the pull.
pub const WASM_SYNC_ANNOUNCE_INTERVAL: Duration = Duration::from_secs(30);

/// Current announce cadence: hot-reloadable via `POST /config` for large
/// clusters / slow networks, re-read every round so an update takes effect
/// on the next cycle.
fn announce_interval() -> Duration {
  crate::runtime_config::current().wasm_sync_announce_interval()
}

/// Providers that have not re-announced within this window are dropped from
/// the provider map (3 announce intervals, mirroring peer liveness).
fn provider_ttl() -> Duration {
  announce_interval().saturating_mul(3)
}

/// One remembered module offer: which nodes currently announce it.
struct ProviderEntry {
  name: String,
  size: u64,
  nodes: HashMap<NodeId, tokio::time::Instant>,
}

pub async fn run_wasm_sync_service(
  self_id: NodeId,
  network: Libp2pNetworkFactory,
  mut shutdown_rx: ShutdownRx,
) {
  let mut announce_rx = super::subscribe_announcements();
  // sha256 -> providers. Keyed by content hash: two versions of one name
  // are distinct offers, and the conflict policy is applied at download
  // planning, not here.
  let mut providers: HashMap<String, ProviderEntry> = HashMap::new();
  // Announce cadence is a runtime-config tunable: sleep the CURRENT
  // interval each round instead of a fixed `interval`, so `POST /config`
  // changes apply from the next round. First round fires immediately.
  let mut next_announce = tokio::time::Instant::now();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping wasm sync service");
        return;
      }
      _ = tokio::time::sleep_until(next_announce) => {
        next_announce = tokio::time::Instant::now() + announce_interval();
        announce_local_inventory(&self_id, &network).await;
        prune_providers(&mut providers);
        sync_missing_modules(&self_id, &network, &mut providers, &mut announce_rx, &mut shutdown_rx)
          .await;
      }
      received = announce_rx.recv() => {
        match received {
          Ok(announcement) => {
            if announcement.node_id == self_id.to_string() {
              continue;
            }
            record_announcement(&mut providers, announcement);
            sync_missing_modules(&self_id, &network, &mut providers, &mut announce_rx, &mut shutdown_rx)
              .await;
          }
          Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            // Fine: announcements are periodic and idempotent; the provider
            // map heals on the next rounds.
            tracing::debug!(skipped, "wasm inventory announcements lagged");
          }
          Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
      }
    }
  }
}

async fn announce_local_inventory(self_id: &NodeId, network: &Libp2pNetworkFactory) {
  let store = WasmModuleStore::from_env_cached();
  let modules = tokio::task::spawn_blocking(move || local_inventory(&store))
    .await
    .unwrap_or_default();
  if modules.is_empty() {
    return;
  }
  let announcement = WasmInventoryAnnouncement {
    node_id: self_id.to_string(),
    modules,
  };
  match sonic_rs::to_vec(&announcement) {
    Ok(data) => {
      if let Err(err) = network.publish_gossipsub(WASM_MODULES_TOPIC, data).await {
        tracing::debug!(error = ?err, "publish wasm inventory failed");
      }
    }
    Err(err) => tracing::warn!(error = ?err, "encode wasm inventory failed"),
  }
}

fn record_announcement(
  providers: &mut HashMap<String, ProviderEntry>,
  announcement: WasmInventoryAnnouncement,
) {
  let now = tokio::time::Instant::now();
  let node = NodeId::new(&announcement.node_id);
  for module in announcement.modules {
    if !is_valid_module_name(&module.name) || module.size == 0 {
      tracing::debug!(node = %node, module = %module.name, "ignore invalid module offer");
      continue;
    }
    providers
      .entry(module.sha256.clone())
      .or_insert_with(|| ProviderEntry {
        name: module.name.clone(),
        size: module.size,
        nodes: HashMap::new(),
      })
      .nodes
      .insert(node.clone(), now);
  }
}

fn prune_providers(providers: &mut HashMap<String, ProviderEntry>) {
  let now = tokio::time::Instant::now();
  let ttl = provider_ttl();
  providers.retain(|_, entry| {
    entry
      .nodes
      .retain(|_, last_seen| now.duration_since(*last_seen) < ttl);
    !entry.nodes.is_empty()
  });
}

/// Offers worth downloading right now: the name is absent from the local
/// store. A name present with a DIFFERENT hash is version skew — logged,
/// never auto-overwritten (see module docs).
fn plan_wanted_modules(
  store: &WasmModuleStore,
  providers: &HashMap<String, ProviderEntry>,
) -> Vec<(String, WasmModuleSummary)> {
  let local: HashSet<String> = store.list().into_iter().collect();
  let local_hashes: HashMap<String, String> = local
    .iter()
    .filter_map(|name| {
      super::manifest_for_file(&store.dir().join(name))
        .ok()
        .map(|manifest| (name.clone(), manifest.sha256))
    })
    .collect();

  let mut wanted = Vec::new();
  for (sha256, entry) in providers {
    match local_hashes.get(&entry.name) {
      None => wanted.push((
        sha256.clone(),
        WasmModuleSummary {
          name: entry.name.clone(),
          sha256: sha256.clone(),
          size: entry.size,
        },
      )),
      Some(local_hash) if local_hash != sha256 => {
        tracing::warn!(
          module = %entry.name,
          local_sha256 = %local_hash,
          announced_sha256 = %sha256,
          "wasm module version skew: peers announce different content for a module we already \
           have; keeping the local file (deploy under a new name to distribute)"
        );
      }
      Some(_) => {}
    }
  }
  // Deterministic order so every round makes progress on the same module
  // first instead of thrashing across many partial downloads.
  wanted.sort_by(|a, b| a.1.name.cmp(&b.1.name).then(a.0.cmp(&b.0)));
  wanted
}

/// Fold announcements that arrived while a download was running into the
/// provider map, without blocking. This is what lets a node that finished
/// (or upgraded) DURING a long download start serving chunks for the rest
/// of it.
fn drain_announcements(
  providers: &mut HashMap<String, ProviderEntry>,
  announce_rx: &mut tokio::sync::broadcast::Receiver<WasmInventoryAnnouncement>,
  self_id: &NodeId,
) {
  loop {
    match announce_rx.try_recv() {
      Ok(announcement) => {
        if announcement.node_id != self_id.to_string() {
          record_announcement(providers, announcement);
        }
      }
      Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
      Err(_) => return,
    }
  }
}

/// Current dialable providers of one module (may change between chunks).
async fn current_sources(
  network: &Libp2pNetworkFactory,
  providers: &HashMap<String, ProviderEntry>,
  sha256: &str,
  self_id: &NodeId,
) -> Vec<NodeId> {
  match providers.get(sha256) {
    Some(entry) => alive_sources(network, entry, self_id).await,
    None => Vec::new(),
  }
}

async fn sync_missing_modules(
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
  providers: &mut HashMap<String, ProviderEntry>,
  announce_rx: &mut tokio::sync::broadcast::Receiver<WasmInventoryAnnouncement>,
  shutdown_rx: &mut ShutdownRx,
) {
  let store = WasmModuleStore::from_env_cached();
  let wanted = plan_wanted_modules(&store, providers);
  for (sha256, summary) in wanted {
    if current_sources(network, providers, &sha256, self_id)
      .await
      .is_empty()
    {
      continue;
    }
    match download_module(
      network,
      &store,
      &summary,
      providers,
      announce_rx,
      self_id,
      shutdown_rx,
    )
    .await
    {
      Ok(DownloadOutcome::Installed(path)) => {
        metrics::counter!("wasm_sync_modules_installed_total").increment(1);
        tracing::info!(
          module = %summary.name,
          sha256 = %summary.sha256,
          path = %path.display(),
          "installed wasm module from p2p sync"
        );
        // Seed immediately: peers still missing the module can start
        // pulling from this node without waiting an announce interval.
        announce_local_inventory(self_id, network).await;
      }
      Ok(DownloadOutcome::Stalled { received, total }) => {
        tracing::info!(
          module = %summary.name,
          sha256 = %summary.sha256,
          received,
          total,
          "wasm module download stalled (providers unreachable); progress persisted, will resume"
        );
      }
      Ok(DownloadOutcome::Shutdown) => return,
      Err(err) => {
        tracing::warn!(
          module = %summary.name,
          sha256 = %summary.sha256,
          error = %err,
          "wasm module download failed"
        );
      }
    }
  }
}

/// Providers of one module worth dialing this round: announced recently and
/// (when their node id parses to a peer id) currently believed alive.
async fn alive_sources(
  network: &Libp2pNetworkFactory,
  entry: &ProviderEntry,
  self_id: &NodeId,
) -> Vec<NodeId> {
  let mut sources = Vec::new();
  for node in entry.nodes.keys() {
    if node == self_id {
      continue;
    }
    // Node ids in this cluster are libp2p peer ids; skip peers that are
    // known-dead to avoid burning a timeout per chunk on them. Unparseable
    // ids stay in — the RPC path resolves them through the address book.
    if let Ok(peer) = node.to_string().parse::<PeerId>()
      && !network.is_peer_alive(&peer).await
    {
      continue;
    }
    sources.push(node.clone());
  }
  sources.sort();
  sources
}

enum DownloadOutcome {
  Installed(std::path::PathBuf),
  /// Some chunks could not be fetched from any provider; partial progress
  /// is on disk and a later round resumes it.
  Stalled {
    received: u32,
    total: u32,
  },
  Shutdown,
}

/// Fetch one module. Chunk `i` is first requested from provider
/// `i % sources.len()` — a deliberate stripe so concurrent downloaders pull
/// different chunks from different seeders — and failures fall through to
/// the remaining providers before the chunk is counted as unfetchable.
///
/// The provider set is refreshed from pending announcements before every
/// chunk, so a peer that completes its own download (and announces) midway
/// starts sharing the remaining transfer load immediately.
async fn download_module(
  network: &Libp2pNetworkFactory,
  store: &WasmModuleStore,
  summary: &WasmModuleSummary,
  providers: &mut HashMap<String, ProviderEntry>,
  announce_rx: &mut tokio::sync::broadcast::Receiver<WasmInventoryAnnouncement>,
  self_id: &NodeId,
  shutdown_rx: &mut ShutdownRx,
) -> Result<DownloadOutcome, String> {
  let sources = current_sources(network, providers, &summary.sha256, self_id).await;
  let manifest = fetch_manifest(network, summary, &sources).await?;
  let mut download = {
    let manifest = manifest.clone();
    let store_dir = store.dir().to_path_buf();
    tokio::task::spawn_blocking(move || PartialDownload::open(&store_dir, manifest))
      .await
      .map_err(|err| format!("open partial download panicked: {err}"))??
  };

  let missing = download.missing_chunks();
  let total = manifest.chunk_count();
  tracing::info!(
    module = %manifest.name,
    sha256 = %manifest.sha256,
    missing = missing.len(),
    total,
    providers = sources.len(),
    "syncing wasm module"
  );

  let mut unfetchable = 0u32;
  for chunk_index in missing {
    if shutdown_rx.has_changed().unwrap_or(true) {
      return Ok(DownloadOutcome::Shutdown);
    }
    // Pick up providers that announced since the last chunk (a peer that
    // just completed this module, a returning seeder).
    drain_announcements(providers, announce_rx, self_id);
    let sources = current_sources(network, providers, &manifest.sha256, self_id).await;
    if sources.is_empty() {
      unfetchable += 1;
      continue;
    }
    let mut fetched = false;
    for attempt in 0 .. sources.len() {
      let source = &sources[(chunk_index as usize + attempt) % sources.len()];
      let request = WasmSyncRequest::Chunk {
        sha256: manifest.sha256.clone(),
        index: chunk_index,
      };
      match network.request_wasm_sync(source.clone(), request).await {
        Ok(WasmSyncResponse::Chunk { index, data, .. }) if index == chunk_index => {
          match download.write_chunk(chunk_index, &data) {
            Ok(()) => {
              metrics::counter!("wasm_sync_chunks_fetched_total").increment(1);
              fetched = true;
              break;
            }
            Err(err) => {
              tracing::debug!(
                source = %source,
                chunk = chunk_index,
                error = %err,
                "wasm chunk rejected; trying next provider"
              );
            }
          }
        }
        Ok(WasmSyncResponse::NotFound) => {
          tracing::debug!(source = %source, chunk = chunk_index, "provider lacks chunk");
        }
        Ok(other) => {
          tracing::debug!(source = %source, chunk = chunk_index, response = ?variant_name(&other), "unexpected wasm sync response");
        }
        Err(err) => {
          tracing::debug!(source = %source, chunk = chunk_index, error = %err, "wasm chunk request failed");
        }
      }
    }
    if !fetched {
      unfetchable += 1;
    }
  }

  if unfetchable > 0 {
    return Ok(DownloadOutcome::Stalled {
      received: download.received_count(),
      total,
    });
  }

  let store_dir = store.dir().to_path_buf();
  let installed = tokio::task::spawn_blocking(move || download.install(&store_dir))
    .await
    .map_err(|err| format!("install panicked: {err}"))??;
  Ok(DownloadOutcome::Installed(installed))
}

async fn fetch_manifest(
  network: &Libp2pNetworkFactory,
  summary: &WasmModuleSummary,
  sources: &[NodeId],
) -> Result<WasmModuleManifest, String> {
  let mut last_error = "no providers".to_string();
  for source in sources {
    let request = WasmSyncRequest::Manifest {
      name: summary.name.clone(),
    };
    match network.request_wasm_sync(source.clone(), request).await {
      Ok(WasmSyncResponse::Manifest(Some(manifest))) => {
        // The manifest must describe exactly the announced content —
        // anything else (provider redeployed meanwhile, bad actor) is
        // skipped, not trusted.
        if manifest.name != summary.name || manifest.sha256 != summary.sha256 {
          last_error = format!("{source}: manifest does not match announcement");
          continue;
        }
        if let Err(err) = manifest.validate() {
          last_error = format!("{source}: invalid manifest: {err}");
          continue;
        }
        return Ok(manifest);
      }
      Ok(WasmSyncResponse::Manifest(None)) => {
        last_error = format!("{source}: module no longer available");
      }
      Ok(other) => {
        last_error = format!("{source}: unexpected response {}", variant_name(&other));
      }
      Err(err) => {
        last_error = format!("{source}: {err}");
      }
    }
  }
  Err(format!(
    "manifest fetch failed from every provider: {last_error}"
  ))
}

fn variant_name(response: &WasmSyncResponse) -> &'static str {
  match response {
    WasmSyncResponse::Manifest(_) => "Manifest",
    WasmSyncResponse::Chunk { .. } => "Chunk",
    WasmSyncResponse::NotFound => "NotFound",
    WasmSyncResponse::Error(_) => "Error",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn announcement(node: &str, modules: &[(&str, &str, u64)]) -> WasmInventoryAnnouncement {
    WasmInventoryAnnouncement {
      node_id: node.to_string(),
      modules: modules
        .iter()
        .map(|(name, sha, size)| WasmModuleSummary {
          name: name.to_string(),
          sha256: sha.to_string(),
          size: *size,
        })
        .collect(),
    }
  }

  #[tokio::test]
  async fn wanted_modules_skip_present_names_and_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("have.wasm"), b"local bytes").unwrap();
    let store = WasmModuleStore::new(dir.path());
    let local_sha = super::super::manifest_for_file(&dir.path().join("have.wasm"))
      .unwrap()
      .sha256;

    let mut providers = HashMap::new();
    // A module we lack, the module we have (same hash), and a conflicting
    // version of the module we have.
    record_announcement(
      &mut providers,
      announcement(
        "peer-a",
        &[
          ("missing.wasm", &"a".repeat(64), 10),
          ("have.wasm", &local_sha, 11),
          ("have.wasm", &"b".repeat(64), 12),
        ],
      ),
    );
    // Invalid offers never enter the provider map.
    record_announcement(
      &mut providers,
      announcement("peer-b", &[("../evil.wasm", &"c".repeat(64), 13)]),
    );

    let wanted = plan_wanted_modules(&store, &providers);
    assert_eq!(wanted.len(), 1);
    assert_eq!(wanted[0].1.name, "missing.wasm");
    assert_eq!(wanted[0].1.sha256, "a".repeat(64));
  }

  #[tokio::test]
  async fn providers_expire_after_ttl() {
    tokio::time::pause();
    let mut providers = HashMap::new();
    record_announcement(
      &mut providers,
      announcement("peer-a", &[("m.wasm", &"a".repeat(64), 10)]),
    );
    prune_providers(&mut providers);
    assert_eq!(providers.len(), 1);

    tokio::time::advance(provider_ttl() + Duration::from_secs(1)).await;
    prune_providers(&mut providers);
    assert!(providers.is_empty());
  }
}
