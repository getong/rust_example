use std::{
  collections::{HashMap, HashSet},
  hash::{DefaultHasher, Hash, Hasher},
  io::Read,
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use libp2p::{
  PeerId,
  gossipsub::{
    TopicHash,
    partial_messages::{Metadata, Partial, PartialAction, PartialError},
  },
};
use openraft::async_runtime::WatchReceiver;
use serde::{Deserialize, Serialize};

use crate::{
  GroupId, Raft,
  network::rpc::RaftRpcResponse,
  openraft_group,
  typ::{LogId, RaftError, Snapshot, SnapshotMeta, Vote},
};

pub const OPENRAFT_SYNC_TOPIC: &str = "openraft/snapshot-sync/1";

const PART_SIZE: usize = 8 * 1024;
const MAX_PARTS_PER_MESSAGE: usize = 4;
const GROUP_ID_LEN: usize = 8;
const MAX_PARTS: usize = u16::MAX as usize;
const METADATA_MAGIC: &[u8; 4] = b"ORS1";
const SNAPSHOT_BUILD_TIMEOUT: Duration = Duration::from_secs(20);
const SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(50);

static NEXT_PARTIAL_GROUP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct OpenRaftSnapshotMetadata {
  group_id: GroupId,
  snapshot_id: String,
  snapshot_size: u64,
  total_parts: u16,
  pub bitmap: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenRaftSnapshotPayload {
  group_id: GroupId,
  vote: Vote,
  meta: SnapshotMeta,
  data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct OpenRaftSnapshotPartial {
  pub group_id: Vec<u8>,
  pub raft_group_id: GroupId,
  pub snapshot_id: String,
  snapshot_size: u64,
  parts: Vec<Option<Vec<u8>>>,
}

impl OpenRaftSnapshotPartial {
  pub async fn from_raft_group(
    group_id: &str,
    local_peer_id: PeerId,
  ) -> anyhow::Result<Option<Self>> {
    let Some(group) = openraft_group(group_id) else {
      anyhow::bail!("unknown group_id={group_id}");
    };

    let metrics = group.raft.metrics().borrow_watched().clone();
    let target = metrics.last_applied.clone();
    let mut snapshot_progress = group.raft.watch_snapshot_progress();

    group
      .raft
      .trigger()
      .snapshot()
      .await
      .map_err(|err| anyhow::anyhow!("trigger openraft snapshot: {err}"))?;

    if let Some(target) = target.as_ref() {
      let target_progress = Some(target.clone());
      let wait = snapshot_progress.wait_until_ge(&target_progress);
      tokio::time::timeout(SNAPSHOT_BUILD_TIMEOUT, wait)
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for openraft snapshot to cover {target}"))?
        .map_err(|err| anyhow::anyhow!("watch openraft snapshot progress: {err}"))?;
    }

    let Some(snapshot) = wait_for_current_snapshot(&group.raft, target.as_ref()).await? else {
      return Ok(None);
    };

    let metrics = group.raft.metrics().borrow_watched().clone();
    let mut data = Vec::new();
    let mut snapshot_reader = snapshot.snapshot;
    snapshot_reader
      .read_to_end(&mut data)
      .map_err(|err| anyhow::anyhow!("read openraft snapshot: {err}"))?;

    let payload = OpenRaftSnapshotPayload {
      group_id: group_id.to_string(),
      vote: metrics.vote,
      meta: snapshot.meta,
      data,
    };
    Self::from_payload(payload, local_peer_id)
  }

  fn from_payload(
    payload: OpenRaftSnapshotPayload,
    local_peer_id: PeerId,
  ) -> anyhow::Result<Option<Self>> {
    let bytes = sonic_rs::to_vec(&payload)?;
    let total_parts = bytes.len().div_ceil(PART_SIZE).max(1);
    if total_parts > MAX_PARTS {
      anyhow::bail!("snapshot needs {total_parts} parts; max supported is {MAX_PARTS}");
    }

    let sequence = NEXT_PARTIAL_GROUP.fetch_add(1, Ordering::Relaxed);
    let mut hasher = DefaultHasher::new();
    local_peer_id.hash(&mut hasher);
    payload.group_id.hash(&mut hasher);
    payload.meta.snapshot_id.hash(&mut hasher);
    bytes.hash(&mut hasher);
    sequence.hash(&mut hasher);

    let parts = if bytes.is_empty() {
      vec![Some(Vec::new())]
    } else {
      bytes
        .chunks(PART_SIZE)
        .map(|chunk| Some(chunk.to_vec()))
        .collect()
    };

    Ok(Some(Self {
      group_id: hasher.finish().to_be_bytes().to_vec(),
      raft_group_id: payload.group_id,
      snapshot_id: payload.meta.snapshot_id,
      snapshot_size: bytes.len() as u64,
      parts,
    }))
  }

  pub fn empty(group_id: Vec<u8>, metadata: &OpenRaftSnapshotMetadata) -> Self {
    Self {
      group_id,
      raft_group_id: metadata.group_id.clone(),
      snapshot_id: metadata.snapshot_id.clone(),
      snapshot_size: metadata.snapshot_size,
      parts: vec![None; metadata.total_parts as usize],
    }
  }

  pub fn parse_metadata(metadata: &[u8]) -> Result<OpenRaftSnapshotMetadata, PartialError> {
    let header_len = Self::metadata_header_len(metadata)?;
    let total_parts = u16::from_be_bytes([metadata[4], metadata[5]]);
    if total_parts == 0 {
      return Err(PartialError::InvalidFormat);
    }

    let snapshot_size = u64::from_be_bytes([
      metadata[6],
      metadata[7],
      metadata[8],
      metadata[9],
      metadata[10],
      metadata[11],
      metadata[12],
      metadata[13],
    ]);
    let group_id_len = u16::from_be_bytes([metadata[14], metadata[15]]) as usize;
    let group_id = std::str::from_utf8(&metadata[16 .. 16 + group_id_len])
      .map_err(|_| PartialError::InvalidFormat)?
      .to_string();
    let snapshot_id_start = 18 + group_id_len;
    let snapshot_id = std::str::from_utf8(&metadata[snapshot_id_start .. header_len])
      .map_err(|_| PartialError::InvalidFormat)?
      .to_string();
    if group_id.is_empty() || snapshot_id.is_empty() {
      return Err(PartialError::InvalidFormat);
    }

    let bitmap_len = (total_parts as usize).div_ceil(8);
    if metadata.len() != header_len + bitmap_len {
      return Err(PartialError::InvalidFormat);
    }

    Ok(OpenRaftSnapshotMetadata {
      group_id,
      snapshot_id,
      snapshot_size,
      total_parts,
      bitmap: metadata[header_len ..].to_vec(),
    })
  }

  pub fn metadata_matches(&self, metadata: &OpenRaftSnapshotMetadata) -> bool {
    self.raft_group_id == metadata.group_id
      && self.snapshot_id == metadata.snapshot_id
      && self.snapshot_size == metadata.snapshot_size
      && self.total_parts() == metadata.total_parts
  }

  pub fn merge_body(&mut self, body: &[u8]) -> Result<bool, PartialError> {
    if body.len() < GROUP_ID_LEN + 2 {
      return Err(PartialError::InvalidFormat);
    }

    let trailer_start = body.len() - GROUP_ID_LEN - 2;
    let total_parts = u16::from_be_bytes([body[trailer_start], body[trailer_start + 1]]);
    let received_group_id = &body[trailer_start + 2 ..];
    if total_parts != self.total_parts() {
      return Err(PartialError::InvalidFormat);
    }
    if received_group_id != self.group_id {
      return Err(PartialError::WrongGroup {
        received: received_group_id.to_vec(),
      });
    }

    let mut offset = 0;
    let mut updated = false;
    while offset < trailer_start {
      if offset + 4 > trailer_start {
        return Err(PartialError::InvalidFormat);
      }

      let index = u16::from_be_bytes([body[offset], body[offset + 1]]) as usize;
      let len = u16::from_be_bytes([body[offset + 2], body[offset + 3]]) as usize;
      offset += 4;

      if index >= self.parts.len() || offset + len > trailer_start {
        return Err(PartialError::OutOfRange);
      }

      if self.parts[index].is_none() {
        self.parts[index] = Some(body[offset .. offset + len].to_vec());
        updated = true;
      }
      offset += len;
    }

    Ok(updated)
  }

  pub fn is_complete(&self) -> bool {
    self.parts.iter().all(Option::is_some)
  }

  pub fn has_part(bitmap: &[u8], index: usize) -> bool {
    bitmap
      .get(index / 8)
      .map(|byte| byte & (1 << (index % 8)) != 0)
      .unwrap_or(false)
  }

  pub fn total_parts(&self) -> u16 {
    self.parts.len() as u16
  }

  pub fn present_parts(&self) -> usize {
    self.parts.iter().filter(|part| part.is_some()).count()
  }

  pub async fn install(&self) -> anyhow::Result<RaftRpcResponse> {
    let payload = self.to_payload()?;
    let Some(group) = openraft_group(&payload.group_id) else {
      anyhow::bail!("unknown group_id={}", payload.group_id);
    };
    let snapshot = Snapshot {
      meta: payload.meta,
      snapshot: std::io::Cursor::new(payload.data),
    };
    let res = group
      .raft
      .install_full_snapshot(payload.vote, snapshot)
      .await
      .map_err(RaftError::Fatal);
    Ok(RaftRpcResponse::FullSnapshot(res))
  }

  fn to_payload(&self) -> anyhow::Result<OpenRaftSnapshotPayload> {
    let mut bytes = Vec::with_capacity(self.snapshot_size as usize);
    for part in &self.parts {
      if let Some(part) = part {
        bytes.extend_from_slice(part);
      } else {
        anyhow::bail!("snapshot is missing parts");
      }
    }
    if bytes.len() as u64 != self.snapshot_size {
      anyhow::bail!(
        "assembled snapshot size mismatch: expected {}, got {}",
        self.snapshot_size,
        bytes.len()
      );
    }
    sonic_rs::from_slice(&bytes).map_err(Into::into)
  }

  fn metadata_bytes(&self) -> Vec<u8> {
    let group_id = self.raft_group_id.as_bytes();
    let snapshot_id = self.snapshot_id.as_bytes();
    let mut metadata =
      Vec::with_capacity(18 + group_id.len() + snapshot_id.len() + self.bitmap_len());
    metadata.extend_from_slice(METADATA_MAGIC);
    metadata.extend_from_slice(&self.total_parts().to_be_bytes());
    metadata.extend_from_slice(&self.snapshot_size.to_be_bytes());
    metadata.extend_from_slice(&(group_id.len() as u16).to_be_bytes());
    metadata.extend_from_slice(group_id);
    metadata.extend_from_slice(&(snapshot_id.len() as u16).to_be_bytes());
    metadata.extend_from_slice(snapshot_id);
    metadata.extend(self.bitmap());
    metadata
  }

  fn metadata_header_len(metadata: &[u8]) -> Result<usize, PartialError> {
    if metadata.len() < 18 || &metadata[0 .. 4] != METADATA_MAGIC {
      return Err(PartialError::InvalidFormat);
    }
    let group_id_len = u16::from_be_bytes([metadata[14], metadata[15]]) as usize;
    let snapshot_len_pos = 16 + group_id_len;
    if metadata.len() < snapshot_len_pos + 2 {
      return Err(PartialError::InvalidFormat);
    }
    let snapshot_id_len =
      u16::from_be_bytes([metadata[snapshot_len_pos], metadata[snapshot_len_pos + 1]]) as usize;
    let header_len = snapshot_len_pos + 2 + snapshot_id_len;
    if metadata.len() < header_len {
      return Err(PartialError::InvalidFormat);
    }
    Ok(header_len)
  }

  fn bitmap_len(&self) -> usize {
    self.parts.len().div_ceil(8)
  }

  fn bitmap(&self) -> Vec<u8> {
    let mut bitmap = vec![0; self.bitmap_len()];
    for (index, part) in self.parts.iter().enumerate() {
      if part.is_some() {
        bitmap[index / 8] |= 1 << (index % 8);
      }
    }
    bitmap
  }

  fn merge_metadata(left: &mut [u8], right: &[u8]) -> Result<bool, PartialError> {
    let left_header_len = Self::metadata_header_len(left)?;
    let right_header_len = Self::metadata_header_len(right)?;
    if left_header_len != right_header_len
      || left.len() != right.len()
      || left[.. left_header_len] != right[.. right_header_len]
    {
      return Err(PartialError::InvalidFormat);
    }

    let mut updated = false;
    for (left, right) in left[left_header_len ..]
      .iter_mut()
      .zip(&right[right_header_len ..])
    {
      let merged = *left | *right;
      updated |= merged != *left;
      *left = merged;
    }
    Ok(updated)
  }

  fn encode_parts_for(
    &self,
    peer_metadata: Option<&[u8]>,
  ) -> Result<Option<Vec<u8>>, PartialError> {
    let requested = match peer_metadata {
      Some(metadata) => {
        let metadata = Self::parse_metadata(metadata)?;
        if !self.metadata_matches(&metadata) {
          return Err(PartialError::InvalidFormat);
        }
        metadata.bitmap
      }
      None => vec![0; self.bitmap_len()],
    };

    let mut body = Vec::new();
    let mut sent_parts = 0;
    for (index, part) in self.parts.iter().enumerate() {
      if Self::has_part(&requested, index) {
        continue;
      }
      let Some(part) = part else {
        continue;
      };
      body.extend_from_slice(&(index as u16).to_be_bytes());
      body.extend_from_slice(&(part.len() as u16).to_be_bytes());
      body.extend_from_slice(part);
      sent_parts += 1;
      if sent_parts >= MAX_PARTS_PER_MESSAGE {
        break;
      }
    }

    if body.is_empty() {
      Ok(None)
    } else {
      body.extend_from_slice(&self.total_parts().to_be_bytes());
      body.extend_from_slice(&self.group_id);
      Ok(Some(body))
    }
  }
}

impl Partial for OpenRaftSnapshotPartial {
  fn group_id(&self) -> Vec<u8> {
    self.group_id.clone()
  }

  fn metadata(&self) -> Box<dyn Metadata> {
    Box::new(OpenRaftSnapshotPartialMetadata {
      bytes: self.metadata_bytes(),
    })
  }

  fn partial_action_from_metadata(
    &self,
    _peer_id: PeerId,
    metadata: Option<&[u8]>,
  ) -> Result<PartialAction, PartialError> {
    let peer_has_useful_data = if let Some(metadata) = metadata {
      let metadata = Self::parse_metadata(metadata)?;
      if !self.metadata_matches(&metadata) {
        return Err(PartialError::InvalidFormat);
      }
      self
        .parts
        .iter()
        .enumerate()
        .any(|(index, part)| part.is_none() && Self::has_part(&metadata.bitmap, index))
    } else {
      false
    };

    Ok(PartialAction {
      need: peer_has_useful_data,
      send: self
        .encode_parts_for(metadata)?
        .map(|body| (body, self.metadata())),
    })
  }
}

#[derive(Debug)]
struct OpenRaftSnapshotPartialMetadata {
  bytes: Vec<u8>,
}

impl Metadata for OpenRaftSnapshotPartialMetadata {
  fn as_slice(&self) -> &[u8] {
    &self.bytes
  }

  fn update(&mut self, data: &[u8]) -> Result<bool, PartialError> {
    OpenRaftSnapshotPartial::merge_metadata(&mut self.bytes, data)
  }

  fn update_from_data(&mut self, data: &[u8]) -> Result<(), PartialError> {
    let trailer_len = 2 + GROUP_ID_LEN;
    if data.len() < trailer_len {
      return Err(PartialError::InvalidFormat);
    }
    let payload_end = data.len() - trailer_len;
    let metadata = OpenRaftSnapshotPartial::parse_metadata(&self.bytes)?;
    let received_total_parts = u16::from_be_bytes([data[payload_end], data[payload_end + 1]]);
    if received_total_parts != metadata.total_parts {
      return Err(PartialError::InvalidFormat);
    }

    let mut update = vec![0; self.bytes.len()];
    let header_len = OpenRaftSnapshotPartial::metadata_header_len(&self.bytes)?;
    update[.. header_len].copy_from_slice(&self.bytes[.. header_len]);

    let mut offset = 0;
    while offset < payload_end {
      if offset + 4 > payload_end {
        return Err(PartialError::InvalidFormat);
      }
      let index = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
      let len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
      offset += 4;
      if index >= metadata.total_parts as usize || offset + len > payload_end {
        return Err(PartialError::OutOfRange);
      }
      update[header_len + index / 8] |= 1 << (index % 8);
      offset += len;
    }

    OpenRaftSnapshotPartial::merge_metadata(&mut self.bytes, &update)?;
    Ok(())
  }
}

#[derive(Default)]
pub struct OpenRaftSyncState {
  partials: HashMap<Vec<u8>, OpenRaftSnapshotPartial>,
  installed: HashSet<Vec<u8>>,
}

impl OpenRaftSyncState {
  pub fn insert_local(&mut self, partial: OpenRaftSnapshotPartial) {
    self.partials.insert(partial.group_id.clone(), partial);
  }

  pub fn known_partials(&self) -> Vec<OpenRaftSnapshotPartial> {
    self.partials.values().cloned().collect()
  }

  pub fn handle_partial(
    &mut self,
    group_id: Vec<u8>,
    metadata: &[u8],
    message: Option<&[u8]>,
  ) -> Result<OpenRaftSyncUpdate, PartialError> {
    let remote_metadata = OpenRaftSnapshotPartial::parse_metadata(metadata)?;
    let partial = self
      .partials
      .entry(group_id.clone())
      .or_insert_with(|| OpenRaftSnapshotPartial::empty(group_id.clone(), &remote_metadata));

    if !partial.metadata_matches(&remote_metadata) {
      return Err(PartialError::InvalidFormat);
    }

    let mut updated = false;
    if let Some(message) = message {
      updated = partial.merge_body(message)?;
    }

    let remote_has_useful_data = partial.parts.iter().enumerate().any(|(index, part)| {
      part.is_none() && OpenRaftSnapshotPartial::has_part(&remote_metadata.bitmap, index)
    });
    let complete = partial.is_complete();
    let first_complete = complete && self.installed.insert(group_id);

    Ok(OpenRaftSyncUpdate {
      partial: partial.clone(),
      should_republish: updated || remote_has_useful_data,
      first_complete,
    })
  }
}

pub struct OpenRaftSyncUpdate {
  pub partial: OpenRaftSnapshotPartial,
  pub should_republish: bool,
  pub first_complete: bool,
}

async fn wait_for_current_snapshot(
  raft: &Raft,
  target: Option<&LogId>,
) -> anyhow::Result<Option<Snapshot>> {
  let deadline = tokio::time::Instant::now() + SNAPSHOT_BUILD_TIMEOUT;
  loop {
    let snapshot = raft
      .get_snapshot()
      .await
      .map_err(|err| anyhow::anyhow!("get openraft snapshot: {err}"))?;

    if snapshot
      .as_ref()
      .is_some_and(|snapshot| snapshot_covers_target(&snapshot.meta, target))
    {
      return Ok(snapshot);
    }

    if tokio::time::Instant::now() >= deadline {
      if let Some(target) = target {
        anyhow::bail!("timed out waiting for readable openraft snapshot covering {target}");
      }
      return Ok(None);
    }

    tokio::time::sleep(SNAPSHOT_POLL_INTERVAL).await;
  }
}

fn snapshot_covers_target(meta: &SnapshotMeta, target: Option<&LogId>) -> bool {
  match target {
    Some(target) => meta
      .last_log_id
      .as_ref()
      .is_some_and(|snapshot_log_id| snapshot_log_id >= target),
    None => true,
  }
}

pub fn hex_id(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sync_topic_hash() -> TopicHash {
  libp2p::gossipsub::IdentTopic::new(OPENRAFT_SYNC_TOPIC).hash()
}
