//! P2P distribution of wasm module files over libp2p.
//!
//! Tasks reference wasm modules by file name in the docker-like module store
//! ([`crate::tasks::wasm_runtime::WasmModuleStore`]). During a rolling
//! upgrade, nodes still on the old release may lack modules that newly
//! enqueued tasks reference. This module closes that gap without any central
//! file server:
//!
//!   - every node periodically announces its module inventory (name + sha256 + size) on the
//!     gossipsub topic [`crate::network::swarm::WASM_MODULES_TOPIC`];
//!   - a node missing an announced module pulls it in fixed-size chunks over the unified RPC
//!     protocol (`WasmSync` kind), rotating chunk requests across EVERY node known to have the
//!     module, so distribution load spreads over the cluster instead of hammering one seeder;
//!   - chunks land in a persistent partial-download file with a received bitmap
//!     ([`downloader::PartialDownload`]); a disconnect, provider crash, or local restart resumes
//!     from the bitmap instead of starting over (断线续传). Each chunk is verified against a
//!     per-chunk xxh3 hash from the manifest on receipt, and the whole file against its sha256
//!     before it is installed;
//!   - a node serves chunks from modules it holds fully AND from its own in-progress partial
//!     downloads, so a half-downloaded node already relays to later joiners (bittorrent-style swarm
//!     propagation); on completion it announces immediately and becomes a full seeder.
//!
//! Version conflicts are deliberately NOT auto-resolved: a module name that
//! already exists locally with a different hash is never overwritten (two
//! releases announcing different content for the same name would otherwise
//! flap forever). Such skew is logged for the operator; only genuinely
//! missing names are fetched.

pub mod downloader;
pub mod service;

use std::{
  path::{Path, PathBuf},
  sync::OnceLock,
  time::SystemTime,
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::tasks::wasm_runtime::{WasmModuleStore, module_hash};

/// Fixed transfer chunk size. Well under the unified-RPC response cap
/// (10 MiB) even after the JSON envelope, and small enough that one lost
/// chunk costs little to re-fetch from another provider.
pub const WASM_SYNC_CHUNK_SIZE: u32 = 256 * 1024;

/// Hard cap on a synced module file. Guards the partial-file preallocation
/// against a malicious or corrupt manifest; sized far above any realistic
/// wasm component.
pub const MAX_WASM_SYNC_MODULE_BYTES: u64 = 256 * 1024 * 1024;

/// Subdirectory of the module store holding in-progress downloads
/// (`<sha256>.part` + `<sha256>.meta.json`).
pub const PARTIAL_SUBDIR: &str = ".partial";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// Everything a downloader needs to fetch and verify one module: total
/// sha256 pins the content, per-chunk xxh3 hashes let every chunk be
/// verified on receipt no matter which peer served it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WasmModuleManifest {
  /// Bare file name in the module store (e.g. `resize.wasm`).
  pub name: String,
  /// Hex sha256 of the whole file — the content identity chunks are
  /// requested under.
  pub sha256: String,
  pub size: u64,
  pub chunk_size: u32,
  /// xxh3_64 of each chunk, in order.
  pub chunk_hashes: Vec<u64>,
}

impl WasmModuleManifest {
  pub fn from_bytes(name: &str, bytes: &[u8]) -> Self {
    let chunk_size = WASM_SYNC_CHUNK_SIZE;
    let chunk_hashes = bytes
      .chunks(chunk_size as usize)
      .map(xxhash_rust::xxh3::xxh3_64)
      .collect();
    Self {
      name: name.to_string(),
      sha256: module_hash(bytes),
      size: bytes.len() as u64,
      chunk_size,
      chunk_hashes,
    }
  }

  pub fn chunk_count(&self) -> u32 {
    self.chunk_hashes.len() as u32
  }

  /// Byte offset of chunk `index` in the file.
  pub fn chunk_offset(&self, index: u32) -> u64 {
    index as u64 * self.chunk_size as u64
  }

  /// Length of chunk `index` (the last chunk is usually short).
  pub fn chunk_len(&self, index: u32) -> usize {
    let offset = self.chunk_offset(index);
    (self.size.saturating_sub(offset)).min(self.chunk_size as u64) as usize
  }

  /// Verify a chunk fetched from an arbitrary peer before trusting it.
  pub fn verify_chunk(&self, index: u32, data: &[u8]) -> bool {
    let Some(expected) = self.chunk_hashes.get(index as usize) else {
      return false;
    };
    data.len() == self.chunk_len(index) && xxhash_rust::xxh3::xxh3_64(data) == *expected
  }

  /// Structural sanity of a manifest received over the network, checked
  /// before any disk space is allocated for it.
  pub fn validate(&self) -> Result<(), String> {
    if !is_valid_module_name(&self.name) {
      return Err(format!("invalid module name {:?}", self.name));
    }
    if self.size == 0 || self.size > MAX_WASM_SYNC_MODULE_BYTES {
      return Err(format!(
        "module size {} outside (0, {MAX_WASM_SYNC_MODULE_BYTES}]",
        self.size
      ));
    }
    if self.chunk_size == 0 || self.chunk_size > WASM_SYNC_CHUNK_SIZE {
      return Err(format!("invalid chunk_size {}", self.chunk_size));
    }
    let expected_chunks = self.size.div_ceil(self.chunk_size as u64);
    if self.chunk_hashes.len() as u64 != expected_chunks {
      return Err(format!(
        "chunk_hashes len {} does not match size {} / chunk_size {}",
        self.chunk_hashes.len(),
        self.size,
        self.chunk_size
      ));
    }
    if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
      return Err(format!("invalid sha256 {:?}", self.sha256));
    }
    Ok(())
  }
}

/// A module name is only ever a bare `.wasm`/`.wat` file name inside the
/// store — anything path-like arriving over the network is rejected before
/// it touches the filesystem.
pub fn is_valid_module_name(name: &str) -> bool {
  !name.is_empty()
    && !name.starts_with('.')
    && !name.contains(['/', '\\'])
    && !name.contains("..")
    && (name.ends_with(".wasm") || name.ends_with(".wat"))
}

// ---------------------------------------------------------------------------
// RPC surface (a kind of the unified /openraft/rpc/2 protocol)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmSyncRequest {
  /// Full manifest of a module this peer announced by name.
  Manifest { name: String },
  /// One chunk of a module identified by its content hash. Served from the
  /// completed store or from the server's own partial download.
  Chunk { sha256: String, index: u32 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WasmSyncResponse {
  /// `None` when the module is not (or no longer) available here.
  Manifest(Option<WasmModuleManifest>),
  /// Chunk bytes travel out-of-band in the RPC envelope's binary frame
  /// (like raft full snapshots), so they never pass through the JSON
  /// encoder; `data` is emptied by the codec on encode and reattached on
  /// decode.
  Chunk {
    sha256: String,
    index: u32,
    data: Vec<u8>,
  },
  /// Chunk (or module) not available here; the caller rotates to the next
  /// provider.
  NotFound,
  Error(String),
}

// ---------------------------------------------------------------------------
// Inventory announcements (gossipsub)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleSummary {
  pub name: String,
  pub sha256: String,
  pub size: u64,
}

/// Periodic per-node module inventory, JSON on the
/// [`crate::network::swarm::WASM_MODULES_TOPIC`] gossipsub topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmInventoryAnnouncement {
  pub node_id: String,
  pub modules: Vec<WasmModuleSummary>,
}

/// Announcements decoded by the swarm loop are handed to the sync service
/// through a broadcast channel (same decoupling as the task-assignment wake
/// channel): the swarm loop never blocks on the service, and a service that
/// is mid-download simply observes announcements late (or lagged — the next
/// periodic announce re-delivers).
static WASM_ANNOUNCE_TX: OnceLock<broadcast::Sender<WasmInventoryAnnouncement>> = OnceLock::new();

fn announce_channel() -> &'static broadcast::Sender<WasmInventoryAnnouncement> {
  WASM_ANNOUNCE_TX.get_or_init(|| broadcast::channel(64).0)
}

/// Called from the gossipsub event handler for every inventory announcement.
pub fn notify_announcement(announcement: WasmInventoryAnnouncement) {
  // No receiver (service not running, e.g. client binaries) is fine.
  let _ = announce_channel().send(announcement);
}

pub fn subscribe_announcements() -> broadcast::Receiver<WasmInventoryAnnouncement> {
  announce_channel().subscribe()
}

// ---------------------------------------------------------------------------
// Server side
// ---------------------------------------------------------------------------

/// Cached manifest of one store file, invalidated by (mtime, size) so a
/// redeployed module is re-hashed but steady-state chunk serving never
/// re-reads more than the requested chunk.
struct CachedManifest {
  mtime: Option<SystemTime>,
  size: u64,
  manifest: WasmModuleManifest,
}

static MANIFEST_CACHE: OnceLock<
  std::sync::Mutex<std::collections::HashMap<PathBuf, CachedManifest>>,
> = OnceLock::new();

fn manifest_cache() -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, CachedManifest>>
{
  MANIFEST_CACHE.get_or_init(Default::default)
}

/// Manifest of a store file, from cache when the file is unchanged.
pub fn manifest_for_file(path: &Path) -> Result<WasmModuleManifest, String> {
  let meta = std::fs::metadata(path).map_err(|err| format!("stat {}: {err}", path.display()))?;
  let mtime = meta.modified().ok();
  let size = meta.len();
  if let Some(cached) = manifest_cache()
    .lock()
    .expect("manifest cache poisoned")
    .get(path)
    .filter(|cached| cached.mtime == mtime && cached.size == size)
  {
    return Ok(cached.manifest.clone());
  }

  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| format!("non-utf8 module file name: {}", path.display()))?
    .to_string();
  let bytes = std::fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
  let manifest = WasmModuleManifest::from_bytes(&name, &bytes);
  manifest_cache()
    .lock()
    .expect("manifest cache poisoned")
    .insert(
      path.to_path_buf(),
      CachedManifest {
        mtime,
        size,
        manifest: manifest.clone(),
      },
    );
  Ok(manifest)
}

/// Local module inventory for announcements: every `.wasm`/`.wat` file in
/// the store with its content hash.
pub fn local_inventory(store: &WasmModuleStore) -> Vec<WasmModuleSummary> {
  store
    .list()
    .into_iter()
    .filter_map(|name| {
      let path = store.dir().join(&name);
      match manifest_for_file(&path) {
        Ok(manifest) => Some(WasmModuleSummary {
          name,
          sha256: manifest.sha256,
          size: manifest.size,
        }),
        Err(err) => {
          tracing::warn!(module = %name, error = %err, "skip unreadable wasm module in inventory");
          None
        }
      }
    })
    .collect()
}

/// Serve one wasm-sync RPC. Disk work runs on the blocking pool — the
/// caller is a spawned dispatch task, but chunk reads must not tie up the
/// async workers under a download fan-in.
pub async fn process_wasm_sync_request(request: WasmSyncRequest) -> WasmSyncResponse {
  let result = tokio::task::spawn_blocking(move || {
    let store = WasmModuleStore::from_env();
    match request {
      WasmSyncRequest::Manifest { name } => serve_manifest(&store, &name),
      WasmSyncRequest::Chunk { sha256, index } => serve_chunk(&store, &sha256, index),
    }
  })
  .await;
  match result {
    Ok(response) => response,
    Err(err) => WasmSyncResponse::Error(format!("wasm sync request panicked: {err}")),
  }
}

fn serve_manifest(store: &WasmModuleStore, name: &str) -> WasmSyncResponse {
  if !is_valid_module_name(name) {
    return WasmSyncResponse::Error(format!("invalid module name {name:?}"));
  }
  let path = store.dir().join(name);
  if !path.is_file() {
    return WasmSyncResponse::Manifest(None);
  }
  match manifest_for_file(&path) {
    Ok(manifest) => WasmSyncResponse::Manifest(Some(manifest)),
    Err(err) => WasmSyncResponse::Error(err),
  }
}

fn serve_chunk(store: &WasmModuleStore, sha256: &str, index: u32) -> WasmSyncResponse {
  if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
    return WasmSyncResponse::Error(format!("invalid sha256 {sha256:?}"));
  }

  // Fully present in the store?
  for name in store.list() {
    let path = store.dir().join(&name);
    let Ok(manifest) = manifest_for_file(&path) else {
      continue;
    };
    if manifest.sha256 != sha256 {
      continue;
    }
    return match read_verified_chunk(&path, &manifest, index) {
      Ok(Some(data)) => {
        metrics::counter!("wasm_sync_chunks_served_total", "source" => "store").increment(1);
        WasmSyncResponse::Chunk {
          sha256: sha256.to_string(),
          index,
          data,
        }
      }
      Ok(None) => WasmSyncResponse::NotFound,
      Err(err) => WasmSyncResponse::Error(err),
    };
  }

  // Mid-download here too? Serve the chunks we already verified, so a
  // downloading node relays to later joiners instead of every joiner
  // queueing on the original seeders.
  match downloader::read_partial_chunk(store.dir(), sha256, index) {
    Ok(Some(data)) => {
      metrics::counter!("wasm_sync_chunks_served_total", "source" => "partial").increment(1);
      WasmSyncResponse::Chunk {
        sha256: sha256.to_string(),
        index,
        data,
      }
    }
    Ok(None) => WasmSyncResponse::NotFound,
    Err(err) => WasmSyncResponse::Error(err),
  }
}

/// Read chunk `index` of `path` and verify it against the manifest (the
/// file may have been swapped since the manifest was cached; never serve
/// bytes that fail their own chunk hash).
fn read_verified_chunk(
  path: &Path,
  manifest: &WasmModuleManifest,
  index: u32,
) -> Result<Option<Vec<u8>>, String> {
  use std::io::{Read, Seek, SeekFrom};

  if index >= manifest.chunk_count() {
    return Ok(None);
  }
  let mut file =
    std::fs::File::open(path).map_err(|err| format!("open {}: {err}", path.display()))?;
  file
    .seek(SeekFrom::Start(manifest.chunk_offset(index)))
    .map_err(|err| format!("seek {}: {err}", path.display()))?;
  let mut data = vec![0u8; manifest.chunk_len(index)];
  file
    .read_exact(&mut data)
    .map_err(|err| format!("read chunk {index} of {}: {err}", path.display()))?;
  if !manifest.verify_chunk(index, &data) {
    return Err(format!(
      "chunk {index} of {} failed verification (file changed?)",
      path.display()
    ));
  }
  Ok(Some(data))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn manifest_chunk_math_covers_short_tail() {
    let bytes: Vec<u8> = (0 .. u8::MAX)
      .cycle()
      .take(WASM_SYNC_CHUNK_SIZE as usize + 100)
      .collect();
    let manifest = WasmModuleManifest::from_bytes("m.wasm", &bytes);
    assert_eq!(manifest.chunk_count(), 2);
    assert_eq!(manifest.chunk_len(0), WASM_SYNC_CHUNK_SIZE as usize);
    assert_eq!(manifest.chunk_len(1), 100);
    assert_eq!(manifest.chunk_offset(1), WASM_SYNC_CHUNK_SIZE as u64);
    assert!(manifest.validate().is_ok());
    assert!(manifest.verify_chunk(0, &bytes[.. WASM_SYNC_CHUNK_SIZE as usize]));
    assert!(manifest.verify_chunk(1, &bytes[WASM_SYNC_CHUNK_SIZE as usize ..]));
    // Wrong bytes and out-of-range chunks must fail.
    assert!(!manifest.verify_chunk(1, &bytes[.. 100]));
    assert!(!manifest.verify_chunk(2, &[]));
  }

  #[test]
  fn manifest_validate_rejects_bad_shapes() {
    let good = WasmModuleManifest::from_bytes("m.wasm", b"abc");

    let mut bad = good.clone();
    bad.name = "../evil.wasm".to_string();
    assert!(bad.validate().is_err());

    let mut bad = good.clone();
    bad.chunk_hashes.push(1);
    assert!(bad.validate().is_err());

    let mut bad = good.clone();
    bad.sha256 = "zz".to_string();
    assert!(bad.validate().is_err());

    let mut bad = good.clone();
    bad.size = MAX_WASM_SYNC_MODULE_BYTES + 1;
    assert!(bad.validate().is_err());

    assert!(good.validate().is_ok());
  }

  #[test]
  fn module_name_validation() {
    assert!(is_valid_module_name("resize.wasm"));
    assert!(is_valid_module_name("hello.wat"));
    assert!(!is_valid_module_name("no_extension"));
    assert!(!is_valid_module_name(".hidden.wasm"));
    assert!(!is_valid_module_name("a/b.wasm"));
    assert!(!is_valid_module_name("a\\b.wasm"));
    assert!(!is_valid_module_name("a..b.wasm"));
    assert!(!is_valid_module_name(""));
  }

  #[test]
  fn serve_manifest_and_chunks_from_store() {
    let dir = tempfile::tempdir().unwrap();
    let bytes: Vec<u8> = (0 .. u8::MAX).cycle().take(1000).collect();
    std::fs::write(dir.path().join("m.wasm"), &bytes).unwrap();
    let store = WasmModuleStore::new(dir.path());

    let WasmSyncResponse::Manifest(Some(manifest)) = serve_manifest(&store, "m.wasm") else {
      panic!("expected manifest");
    };
    assert_eq!(manifest.size, 1000);
    assert_eq!(manifest.chunk_count(), 1);

    let WasmSyncResponse::Chunk { data, index, .. } = serve_chunk(&store, &manifest.sha256, 0)
    else {
      panic!("expected chunk");
    };
    assert_eq!(index, 0);
    assert_eq!(data, bytes);

    // Unknown hash and out-of-range chunk answer NotFound, not an error.
    let unknown = "0".repeat(64);
    assert!(matches!(
      serve_chunk(&store, &unknown, 0),
      WasmSyncResponse::NotFound
    ));
    assert!(matches!(
      serve_chunk(&store, &manifest.sha256, 5),
      WasmSyncResponse::NotFound
    ));

    // Path-shaped names are rejected outright.
    assert!(matches!(
      serve_manifest(&store, "../m.wasm"),
      WasmSyncResponse::Error(_)
    ));
  }
}
