use std::{
  net::{Ipv4Addr, SocketAddr},
  str::FromStr,
  time::Duration,
};

use iroh_blobs::{BlobFormat, Hash};
use iroh_gossip::TopicId;
use n0_error::{Result, StdResultExt};

pub(crate) fn parse_secret_key(input: &str) -> Result<[u8; 32]> {
  parse_hex_32(input, "cluster secret")
}

pub(crate) fn parse_iroh_secret_key(input: &str) -> Result<[u8; 32]> {
  parse_hex_32(input, "iroh secret key")
}

pub(crate) fn parse_hex_32(input: &str, field: &str) -> Result<[u8; 32]> {
  let input = input.trim();
  let mut bytes = [0u8; 32];

  if input.len() != 64 {
    return Err(n0_error::anyerr!("{field} must be 64 hex characters"));
  }

  for (index, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
    let part = std::str::from_utf8(chunk).anyerr()?;
    bytes[index] = u8::from_str_radix(part, 16).anyerr()?;
  }

  Ok(bytes)
}

pub fn parse_gossip_topic(input: &str) -> Result<TopicId> {
  Ok(TopicId::from_bytes(parse_hex_32(input, "gossip topic")?))
}

pub fn parse_blob_hash(input: &str) -> Result<Hash> {
  Hash::from_str(input).anyerr()
}

pub(crate) fn parse_blob_format(input: &str) -> Result<BlobFormat> {
  match input {
    "raw" => Ok(BlobFormat::Raw),
    "hash-seq" => Ok(BlobFormat::HashSeq),
    other => Err(n0_error::anyerr!("unsupported blob format {other}")),
  }
}

pub(crate) fn blob_format_name(format: BlobFormat) -> &'static str {
  match format {
    BlobFormat::Raw => "raw",
    BlobFormat::HashSeq => "hash-seq",
  }
}

pub fn parse_bootstrap(values: &[String]) -> Vec<String> {
  values
    .iter()
    .flat_map(|value| value.split(','))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToString::to_string)
    .collect()
}

pub fn parse_duration_secs(seconds: u64) -> Duration {
  Duration::from_secs(seconds)
}

/// Parse and validate republish interval; must be at least 30 seconds.
pub fn parse_republish_interval(seconds: u64) -> Duration {
  Duration::from_secs(seconds.max(30))
}

pub fn parse_socket_addr(value: &str) -> Result<SocketAddr> {
  SocketAddr::from_str(value).anyerr()
}

pub fn parse_ipv4(value: &str) -> Result<Ipv4Addr> {
  Ipv4Addr::from_str(value).anyerr()
}

pub fn parse_dht_port(port: u16) -> Option<u16> {
  (port != 0).then_some(port)
}

/// Validate the output path has no path-traversal components and its parent exists.
pub(crate) fn validate_output_path(
  output: &std::path::Path,
  store_path: &std::path::Path,
) -> Result<()> {
  if output
    .components()
    .any(|c| c == std::path::Component::ParentDir)
  {
    return Err(n0_error::anyerr!(
      "output path '{}' contains '..' traversal component",
      output.display()
    ));
  }
  if output.is_absolute() && !output.starts_with(store_path) {
    return Err(n0_error::anyerr!(
      "output path '{}' is outside store directory '{}'; use a path inside the store directory",
      output.display(),
      store_path.display()
    ));
  }
  if let Some(parent) = output.parent()
    && !parent.as_os_str().is_empty()
    && !parent.exists()
  {
    return Err(n0_error::anyerr!(
      "output parent directory does not exist: {}",
      parent.display()
    ));
  }
  Ok(())
}
