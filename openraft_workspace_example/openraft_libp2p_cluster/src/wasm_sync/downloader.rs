//! Persistent, resumable partial downloads (断线续传).
//!
//! One in-progress module download is two files under
//! `<store>/.partial/`, keyed by content hash so concurrent versions of the
//! same name never collide:
//!
//!   - `<sha256>.part` — the module bytes, preallocated to full size and written per chunk at the
//!     chunk's offset;
//!   - `<sha256>.meta.json` — the manifest plus the set of received chunk indexes, rewritten after
//!     every accepted chunk.
//!
//! Resume is trust-nothing: on open, every chunk the meta file claims is
//! present is re-verified against its xxh3 hash (a crash between the part
//! write and the meta write, or a truncated part file, degrades to "chunk
//! missing", never to corrupt data). Installation verifies the whole file's
//! sha256 and moves it into the store atomically (same-directory temp +
//! rename), so the store never exposes a half-written module.

use std::{
  collections::BTreeSet,
  io::{Read, Seek, SeekFrom, Write},
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{PARTIAL_SUBDIR, WasmModuleManifest};
use crate::tasks::wasm_runtime::module_hash;

#[derive(Debug, Serialize, Deserialize)]
struct PartialMeta {
  manifest: WasmModuleManifest,
  /// Received chunk indexes. A sorted set (not a bitmap) keeps the file
  /// human-inspectable; chunk counts are small (size / 256 KiB).
  received: BTreeSet<u32>,
}

pub struct PartialDownload {
  manifest: WasmModuleManifest,
  part_path: PathBuf,
  meta_path: PathBuf,
  received: BTreeSet<u32>,
}

fn partial_dir(store_dir: &Path) -> PathBuf {
  store_dir.join(PARTIAL_SUBDIR)
}

fn part_path(store_dir: &Path, sha256: &str) -> PathBuf {
  partial_dir(store_dir).join(format!("{sha256}.part"))
}

fn meta_path(store_dir: &Path, sha256: &str) -> PathBuf {
  partial_dir(store_dir).join(format!("{sha256}.meta.json"))
}

impl PartialDownload {
  /// Open (resuming any prior progress) or create the partial download for
  /// `manifest`. The manifest must already be validated.
  pub fn open(store_dir: &Path, manifest: WasmModuleManifest) -> Result<Self, String> {
    let dir = partial_dir(store_dir);
    std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;

    let part_path = part_path(store_dir, &manifest.sha256);
    let meta_path = meta_path(store_dir, &manifest.sha256);

    let mut download = Self {
      manifest,
      part_path,
      meta_path,
      received: BTreeSet::new(),
    };

    match download.load_and_verify_meta() {
      Ok(recovered) => download.received = recovered,
      Err(err) => {
        tracing::warn!(
          part = %download.part_path.display(),
          error = %err,
          "partial download state unusable; restarting from zero"
        );
      }
    }

    // (Re)size the part file: full size up front so chunk writes at any
    // offset succeed and disk-full fails the download now, not at chunk N.
    let file = std::fs::OpenOptions::new()
      .create(true)
      .write(true)
      .truncate(false)
      .open(&download.part_path)
      .map_err(|err| format!("open {}: {err}", download.part_path.display()))?;
    file
      .set_len(download.manifest.size)
      .map_err(|err| format!("resize {}: {err}", download.part_path.display()))?;

    download.persist_meta()?;
    Ok(download)
  }

  /// Received chunks recorded by a previous run, re-verified chunk by chunk
  /// against the part file. Any mismatch (missing files, different
  /// manifest, bad bytes) demotes only the affected chunks.
  fn load_and_verify_meta(&self) -> Result<BTreeSet<u32>, String> {
    if !self.meta_path.is_file() || !self.part_path.is_file() {
      return Ok(BTreeSet::new());
    }
    let raw = std::fs::read_to_string(&self.meta_path)
      .map_err(|err| format!("read {}: {err}", self.meta_path.display()))?;
    let meta: PartialMeta =
      sonic_rs::from_str(&raw).map_err(|err| format!("decode partial meta: {err}"))?;
    if meta.manifest != self.manifest {
      return Err("stored manifest differs from requested manifest".to_string());
    }

    let mut file = std::fs::File::open(&self.part_path)
      .map_err(|err| format!("open {}: {err}", self.part_path.display()))?;
    let mut verified = BTreeSet::new();
    for index in meta.received {
      if index >= self.manifest.chunk_count() {
        continue;
      }
      let mut data = vec![0u8; self.manifest.chunk_len(index)];
      let readable = file
        .seek(SeekFrom::Start(self.manifest.chunk_offset(index)))
        .and_then(|_| file.read_exact(&mut data))
        .is_ok();
      if readable && self.manifest.verify_chunk(index, &data) {
        verified.insert(index);
      }
    }
    Ok(verified)
  }

  fn persist_meta(&self) -> Result<(), String> {
    let meta = PartialMeta {
      manifest: self.manifest.clone(),
      received: self.received.clone(),
    };
    let raw = sonic_rs::to_string(&meta).map_err(|err| format!("encode partial meta: {err}"))?;
    // Same-directory temp + rename so a crash never leaves a truncated
    // meta file (a torn meta would discard all resume progress).
    let tmp = self.meta_path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| format!("write {}: {err}", tmp.display()))?;
    std::fs::rename(&tmp, &self.meta_path)
      .map_err(|err| format!("rename {}: {err}", self.meta_path.display()))
  }

  pub fn manifest(&self) -> &WasmModuleManifest {
    &self.manifest
  }

  pub fn received_count(&self) -> u32 {
    self.received.len() as u32
  }

  pub fn is_complete(&self) -> bool {
    self.received_count() == self.manifest.chunk_count()
  }

  pub fn missing_chunks(&self) -> Vec<u32> {
    (0 .. self.manifest.chunk_count())
      .filter(|index| !self.received.contains(index))
      .collect()
  }

  /// Verify and persist one chunk. Rejecting a bad chunk is not fatal —
  /// the caller re-fetches it from another provider.
  pub fn write_chunk(&mut self, index: u32, data: &[u8]) -> Result<(), String> {
    if index >= self.manifest.chunk_count() {
      return Err(format!(
        "chunk index {index} out of range ({} chunks)",
        self.manifest.chunk_count()
      ));
    }
    if self.received.contains(&index) {
      return Ok(());
    }
    if !self.manifest.verify_chunk(index, data) {
      return Err(format!("chunk {index} failed hash verification"));
    }

    let mut file = std::fs::OpenOptions::new()
      .write(true)
      .open(&self.part_path)
      .map_err(|err| format!("open {}: {err}", self.part_path.display()))?;
    file
      .seek(SeekFrom::Start(self.manifest.chunk_offset(index)))
      .map_err(|err| format!("seek {}: {err}", self.part_path.display()))?;
    file
      .write_all(data)
      .map_err(|err| format!("write chunk {index}: {err}"))?;
    // Chunk bytes reach disk before the meta file claims them; the reverse
    // order could resume from a meta that lies about a lost page. (Verified
    // resume would still catch it, but never on purpose.)
    file
      .sync_data()
      .map_err(|err| format!("sync {}: {err}", self.part_path.display()))?;

    self.received.insert(index);
    self.persist_meta()
  }

  /// Verify the completed file end to end and install it into the store
  /// under its manifest name. Refuses if the name has appeared in the store
  /// meanwhile (never overwrite).
  pub fn install(self, store_dir: &Path) -> Result<PathBuf, String> {
    if !self.is_complete() {
      return Err(format!(
        "download incomplete: {}/{} chunks",
        self.received_count(),
        self.manifest.chunk_count()
      ));
    }
    let bytes = std::fs::read(&self.part_path)
      .map_err(|err| format!("read {}: {err}", self.part_path.display()))?;
    if bytes.len() as u64 != self.manifest.size {
      return Err(format!(
        "part file is {} bytes, manifest says {}",
        bytes.len(),
        self.manifest.size
      ));
    }
    let actual = module_hash(&bytes);
    if actual != self.manifest.sha256 {
      // Poisoned beyond repair (every chunk verified but the whole does
      // not): drop the partial state so the next attempt starts clean.
      self.discard();
      return Err(format!(
        "sha256 mismatch after download: expected {}, got {actual}",
        self.manifest.sha256
      ));
    }

    let final_path = store_dir.join(&self.manifest.name);
    if final_path.exists() {
      self.discard();
      return Err(format!(
        "module {} appeared in the store during download; not overwriting",
        self.manifest.name
      ));
    }
    let tmp = store_dir.join(format!(".{}.installing", self.manifest.sha256));
    std::fs::write(&tmp, &bytes).map_err(|err| format!("write {}: {err}", tmp.display()))?;
    std::fs::rename(&tmp, &final_path)
      .map_err(|err| format!("rename into {}: {err}", final_path.display()))?;

    self.discard();
    Ok(final_path)
  }

  /// Remove the partial files (best effort).
  pub fn discard(&self) {
    let _ = std::fs::remove_file(&self.part_path);
    let _ = std::fs::remove_file(&self.meta_path);
  }
}

/// Serve chunk `index` of an in-progress download, if this node has already
/// received and verified it. `Ok(None)` when there is no such partial or
/// the chunk is still missing.
pub fn read_partial_chunk(
  store_dir: &Path,
  sha256: &str,
  index: u32,
) -> Result<Option<Vec<u8>>, String> {
  let meta_path = meta_path(store_dir, sha256);
  let part_path = part_path(store_dir, sha256);
  if !meta_path.is_file() || !part_path.is_file() {
    return Ok(None);
  }
  let raw = std::fs::read_to_string(&meta_path)
    .map_err(|err| format!("read {}: {err}", meta_path.display()))?;
  let meta: PartialMeta =
    sonic_rs::from_str(&raw).map_err(|err| format!("decode partial meta: {err}"))?;
  if meta.manifest.sha256 != sha256 || !meta.received.contains(&index) {
    return Ok(None);
  }

  let mut file = std::fs::File::open(&part_path)
    .map_err(|err| format!("open {}: {err}", part_path.display()))?;
  file
    .seek(SeekFrom::Start(meta.manifest.chunk_offset(index)))
    .map_err(|err| format!("seek {}: {err}", part_path.display()))?;
  let mut data = vec![0u8; meta.manifest.chunk_len(index)];
  file
    .read_exact(&mut data)
    .map_err(|err| format!("read chunk {index}: {err}"))?;
  if !meta.manifest.verify_chunk(index, &data) {
    // Never relay bytes that fail their own hash.
    return Ok(None);
  }
  Ok(Some(data))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::wasm_sync::WASM_SYNC_CHUNK_SIZE;

  fn test_module() -> Vec<u8> {
    (0 .. u8::MAX)
      .cycle()
      .take(WASM_SYNC_CHUNK_SIZE as usize * 2 + 300)
      .collect()
  }

  fn chunk(manifest: &WasmModuleManifest, bytes: &[u8], index: u32) -> Vec<u8> {
    let start = manifest.chunk_offset(index) as usize;
    bytes[start .. start + manifest.chunk_len(index)].to_vec()
  }

  #[test]
  fn download_resumes_from_persisted_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = test_module();
    let manifest = WasmModuleManifest::from_bytes("m.wasm", &bytes);
    assert_eq!(manifest.chunk_count(), 3);

    // First session: two of three chunks arrive, then the "connection
    // drops" (the download is simply dropped).
    {
      let mut download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();
      download
        .write_chunk(0, &chunk(&manifest, &bytes, 0))
        .unwrap();
      download
        .write_chunk(2, &chunk(&manifest, &bytes, 2))
        .unwrap();
      assert!(!download.is_complete());
    }

    // The partial's verified chunks are servable to OTHER peers already.
    assert_eq!(
      read_partial_chunk(dir.path(), &manifest.sha256, 0).unwrap(),
      Some(chunk(&manifest, &bytes, 0))
    );
    assert_eq!(
      read_partial_chunk(dir.path(), &manifest.sha256, 1).unwrap(),
      None
    );

    // Second session resumes: only chunk 1 is missing, and finishing
    // installs the verified file into the store.
    let mut download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();
    assert_eq!(download.missing_chunks(), vec![1]);
    download
      .write_chunk(1, &chunk(&manifest, &bytes, 1))
      .unwrap();
    assert!(download.is_complete());
    let installed = download.install(dir.path()).unwrap();
    assert_eq!(std::fs::read(&installed).unwrap(), bytes);
    // Partial state is gone after install.
    assert!(!part_path(dir.path(), &manifest.sha256).exists());
    assert!(!meta_path(dir.path(), &manifest.sha256).exists());
  }

  #[test]
  fn corrupt_part_bytes_demote_chunk_on_resume() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = test_module();
    let manifest = WasmModuleManifest::from_bytes("m.wasm", &bytes);

    {
      let mut download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();
      download
        .write_chunk(0, &chunk(&manifest, &bytes, 0))
        .unwrap();
    }

    // Flip a byte inside chunk 0 on disk (torn write / bit rot).
    let part = part_path(dir.path(), &manifest.sha256);
    let mut on_disk = std::fs::read(&part).unwrap();
    on_disk[10] ^= 0xff;
    std::fs::write(&part, &on_disk).unwrap();

    let download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();
    assert_eq!(
      download.missing_chunks(),
      vec![0, 1, 2],
      "corrupt chunk must be re-fetched, not trusted"
    );
    // And it must not be served to peers either.
    assert_eq!(
      read_partial_chunk(dir.path(), &manifest.sha256, 0).unwrap(),
      None
    );
  }

  #[test]
  fn bad_chunk_from_peer_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = test_module();
    let manifest = WasmModuleManifest::from_bytes("m.wasm", &bytes);
    let mut download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();

    let mut bad = chunk(&manifest, &bytes, 0);
    bad[0] ^= 0xff;
    assert!(download.write_chunk(0, &bad).is_err());
    assert_eq!(download.received_count(), 0);
  }

  #[test]
  fn install_refuses_to_overwrite_existing_module() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = test_module();
    let manifest = WasmModuleManifest::from_bytes("m.wasm", &bytes);
    let mut download = PartialDownload::open(dir.path(), manifest.clone()).unwrap();
    for index in 0 .. manifest.chunk_count() {
      download
        .write_chunk(index, &chunk(&manifest, &bytes, index))
        .unwrap();
    }

    // Someone (operator, another sync round) installs the name first.
    std::fs::write(dir.path().join("m.wasm"), b"already here").unwrap();
    assert!(download.install(dir.path()).is_err());
    assert_eq!(
      std::fs::read(dir.path().join("m.wasm")).unwrap(),
      b"already here"
    );
  }
}
