use std::{
  hash::{DefaultHasher, Hash, Hasher},
  io::ErrorKind,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
};

use libp2p::{
  PeerId,
  gossipsub::partial_messages::{Metadata, Partial, PartialAction, PartialError},
};
use tokio::io::AsyncWriteExt;

const PART_SIZE: usize = 8 * 1024;
const MAX_PARTS_PER_MESSAGE: usize = 4;
const GROUP_ID_LEN: usize = 8;
const MAX_PARTS: usize = u16::MAX as usize;
const METADATA_MAGIC: &[u8; 4] = b"FPM1";

static NEXT_PARTIAL_GROUP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub(crate) struct FileMetadata {
  file_name: String,
  file_size: u64,
  total_parts: u16,
  pub(crate) bitmap: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct FilePartialMessage {
  pub(crate) group_id: Vec<u8>,
  pub(crate) file_name: String,
  pub(crate) file_size: u64,
  pub(crate) parts: Vec<Option<Vec<u8>>>,
}

#[derive(Debug)]
pub(crate) enum FileWriteOutcome {
  Written(PathBuf),
  Duplicate(PathBuf),
}

impl FilePartialMessage {
  pub(crate) async fn from_path(path: &Path, local_peer_id: PeerId) -> anyhow::Result<Self> {
    let bytes = tokio::fs::read(path).await?;
    let total_parts = bytes.len().div_ceil(PART_SIZE).max(1);
    if total_parts > MAX_PARTS {
      anyhow::bail!("file needs {total_parts} parts; max supported is {MAX_PARTS}");
    }

    let file_name = path
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| anyhow::anyhow!("file path has no valid UTF-8 file name"))?
      .to_string();
    if file_name.len() > u16::MAX as usize {
      anyhow::bail!("file name is too long");
    }

    let sequence = NEXT_PARTIAL_GROUP.fetch_add(1, Ordering::Relaxed);
    let mut hasher = DefaultHasher::new();
    local_peer_id.hash(&mut hasher);
    file_name.hash(&mut hasher);
    bytes.hash(&mut hasher);
    sequence.hash(&mut hasher);
    let group_id = hasher.finish().to_be_bytes().to_vec();
    let parts = if bytes.is_empty() {
      vec![Some(Vec::new())]
    } else {
      bytes
        .chunks(PART_SIZE)
        .map(|chunk| Some(chunk.to_vec()))
        .collect()
    };

    Ok(Self {
      group_id,
      file_name,
      file_size: bytes.len() as u64,
      parts,
    })
  }

  pub(crate) fn empty(group_id: Vec<u8>, metadata: &FileMetadata) -> Self {
    Self {
      group_id,
      file_name: metadata.file_name.clone(),
      file_size: metadata.file_size,
      parts: vec![None; metadata.total_parts as usize],
    }
  }

  fn metadata_bytes(&self) -> Vec<u8> {
    let file_name = self.file_name.as_bytes();
    let mut metadata = Vec::with_capacity(16 + file_name.len() + self.bitmap_len());
    metadata.extend_from_slice(METADATA_MAGIC);
    metadata.extend_from_slice(&self.total_parts().to_be_bytes());
    metadata.extend_from_slice(&self.file_size.to_be_bytes());
    metadata.extend_from_slice(&(file_name.len() as u16).to_be_bytes());
    metadata.extend_from_slice(file_name);
    metadata.extend(self.bitmap());
    metadata
  }

  pub(crate) fn total_parts(&self) -> u16 {
    self.parts.len() as u16
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

  pub(crate) fn has_part(bitmap: &[u8], index: usize) -> bool {
    bitmap
      .get(index / 8)
      .map(|byte| byte & (1 << (index % 8)) != 0)
      .unwrap_or(false)
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

  pub(crate) fn metadata_matches(&self, metadata: &FileMetadata) -> bool {
    self.file_name == metadata.file_name
      && self.file_size == metadata.file_size
      && self.total_parts() == metadata.total_parts
  }

  fn metadata_header_len(metadata: &[u8]) -> Result<usize, PartialError> {
    if metadata.len() < 16 || &metadata[0 .. 4] != METADATA_MAGIC {
      return Err(PartialError::InvalidFormat);
    }

    let file_name_len = u16::from_be_bytes([metadata[14], metadata[15]]) as usize;
    let header_len = 16 + file_name_len;
    if metadata.len() < header_len {
      return Err(PartialError::InvalidFormat);
    }
    Ok(header_len)
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

  pub(crate) fn merge_body(&mut self, body: &[u8]) -> Result<bool, PartialError> {
    if body.len() < self.group_id.len() + 2 {
      return Err(PartialError::InvalidFormat);
    }

    let trailer_start = body.len() - self.group_id.len() - 2;
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

  pub(crate) fn is_complete(&self) -> bool {
    self.parts.iter().all(Option::is_some)
  }

  fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(self.file_size as usize);
    for part in &self.parts {
      if let Some(part) = part {
        bytes.extend_from_slice(part);
      } else {
        anyhow::bail!("file is missing parts");
      }
    }

    if bytes.len() as u64 != self.file_size {
      anyhow::bail!(
        "assembled file size mismatch: expected {}, got {}",
        self.file_size,
        bytes.len()
      );
    }
    Ok(bytes)
  }

  fn output_path(content_hash: &str) -> PathBuf {
    PathBuf::from("received").join(content_hash)
  }

  pub(crate) async fn write_to_disk(&self) -> anyhow::Result<FileWriteOutcome> {
    let bytes = self.to_bytes()?;
    let content_hash = blake3::hash(&bytes).to_hex().to_string();
    let output_path = Self::output_path(&content_hash);

    tokio::fs::create_dir_all("received").await?;
    let mut file = match tokio::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&output_path)
      .await
    {
      Ok(file) => file,
      Err(error) if error.kind() == ErrorKind::AlreadyExists => {
        return Ok(FileWriteOutcome::Duplicate(output_path));
      }
      Err(error) => return Err(error.into()),
    };

    file.write_all(&bytes).await?;
    Ok(FileWriteOutcome::Written(output_path))
  }

  pub(crate) fn parse_metadata(metadata: &[u8]) -> Result<FileMetadata, PartialError> {
    let header_len = Self::metadata_header_len(metadata)?;

    let total_parts = u16::from_be_bytes([metadata[4], metadata[5]]);
    if total_parts == 0 {
      return Err(PartialError::InvalidFormat);
    }

    let file_size = u64::from_be_bytes([
      metadata[6],
      metadata[7],
      metadata[8],
      metadata[9],
      metadata[10],
      metadata[11],
      metadata[12],
      metadata[13],
    ]);
    let file_name = std::str::from_utf8(&metadata[16 .. header_len])
      .map_err(|_| PartialError::InvalidFormat)?
      .to_string();
    if file_name.is_empty() {
      return Err(PartialError::InvalidFormat);
    }

    let bitmap_len = (total_parts as usize).div_ceil(8);
    if metadata.len() != header_len + bitmap_len {
      return Err(PartialError::InvalidFormat);
    }

    Ok(FileMetadata {
      file_name,
      file_size,
      total_parts,
      bitmap: metadata[header_len ..].to_vec(),
    })
  }
}

impl Partial for FilePartialMessage {
  fn group_id(&self) -> Vec<u8> {
    self.group_id.clone()
  }

  fn metadata(&self) -> Box<dyn Metadata> {
    Box::new(FilePartialMetadata {
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
struct FilePartialMetadata {
  bytes: Vec<u8>,
}

impl Metadata for FilePartialMetadata {
  fn as_slice(&self) -> &[u8] {
    &self.bytes
  }

  fn update(&mut self, data: &[u8]) -> Result<bool, PartialError> {
    FilePartialMessage::merge_metadata(&mut self.bytes, data)
  }

  fn update_from_data(&mut self, data: &[u8]) -> Result<(), PartialError> {
    let trailer_len = 2 + GROUP_ID_LEN;
    if data.len() < trailer_len {
      return Err(PartialError::InvalidFormat);
    }
    let payload_end = data.len() - trailer_len;
    let metadata = FilePartialMessage::parse_metadata(&self.bytes)?;
    let received_total_parts = u16::from_be_bytes([data[payload_end], data[payload_end + 1]]);
    if received_total_parts != metadata.total_parts {
      return Err(PartialError::InvalidFormat);
    }

    let mut update = vec![0; self.bytes.len()];
    let header_len = FilePartialMessage::metadata_header_len(&self.bytes)?;
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

    FilePartialMessage::merge_metadata(&mut self.bytes, &update)?;
    Ok(())
  }
}

pub(crate) fn hex_id(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
