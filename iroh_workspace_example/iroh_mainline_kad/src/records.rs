use std::{collections::BTreeMap, net::SocketAddr, str::FromStr};

use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, TransportAddr};
use iroh_blobs::{BlobFormat, Hash, HashAndFormat};
use n0_error::{Result, StdResultExt};
use serde::{Deserialize, Serialize};

use crate::{
  parsing::{blob_format_name, parse_blob_format},
  protocols::{BLOB_PROTOCOL, GOSSIP_PROTOCOL, REQUEST_PROTOCOL},
  util::now_unix_secs,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRecord {
  pub version: u8,
  pub updated_at: u64,
  pub members: Vec<MemberRecord>,
}

impl ClusterRecord {
  pub(crate) fn new() -> Self {
    Self {
      version: 1,
      updated_at: now_unix_secs(),
      members: Vec::new(),
    }
  }

  pub(crate) fn insert_member(&mut self, member: MemberRecord, max_members: usize) {
    let mut members = BTreeMap::<String, MemberRecord>::new();

    for existing in self.members.drain(..) {
      members.insert(existing.endpoint_id.clone(), existing);
    }

    members.insert(member.endpoint_id.clone(), member);
    self.members = members.into_values().collect();
    self
      .members
      .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    self.members.truncate(max_members);
    self.updated_at = now_unix_secs();
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRecord {
  pub endpoint_id: String,
  pub name: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub protocols: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub blobs: Vec<BlobProviderRecord>,
  pub addrs: Vec<String>,
  pub relay_urls: Vec<String>,
  pub updated_at: u64,
}

impl MemberRecord {
  pub fn endpoint_addr(&self) -> Result<EndpointAddr> {
    let id = EndpointId::from_str(&self.endpoint_id).anyerr()?;
    let mut addrs = Vec::new();

    for addr in &self.addrs {
      addrs.push(TransportAddr::Ip(SocketAddr::from_str(addr).anyerr()?));
    }

    for relay_url in &self.relay_urls {
      addrs.push(TransportAddr::Relay(
        RelayUrl::from_str(relay_url).anyerr()?,
      ));
    }

    Ok(EndpointAddr::from_parts(id, addrs))
  }

  pub(crate) fn supports_request(&self) -> bool {
    self.protocols.is_empty()
      || self
        .protocols
        .iter()
        .any(|protocol| protocol == REQUEST_PROTOCOL)
  }

  pub(crate) fn supports_gossip(&self) -> bool {
    self
      .protocols
      .iter()
      .any(|protocol| protocol == GOSSIP_PROTOCOL)
  }

  pub(crate) fn supports_blob(&self) -> bool {
    self
      .protocols
      .iter()
      .any(|protocol| protocol == BLOB_PROTOCOL)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobProviderRecord {
  pub hash: String,
  pub format: String,
  pub name: String,
  pub size: u64,
}

impl BlobProviderRecord {
  pub(crate) fn hash_and_format(&self) -> Result<HashAndFormat> {
    let hash = Hash::from_str(&self.hash).anyerr()?;
    let format = parse_blob_format(&self.format)?;
    Ok(HashAndFormat::new(hash, format))
  }
}

pub(crate) fn member_from_endpoint(
  endpoint: &Endpoint,
  name: &str,
  protocols: &[&str],
) -> MemberRecord {
  member_from_endpoint_with_blobs(endpoint, name, protocols, Vec::new())
}

pub(crate) fn member_from_endpoint_with_blobs(
  endpoint: &Endpoint,
  name: &str,
  protocols: &[&str],
  blobs: Vec<BlobProviderRecord>,
) -> MemberRecord {
  let addr = endpoint.addr();
  let addrs = addr.ip_addrs().map(ToString::to_string).collect::<Vec<_>>();
  let relay_urls = addr
    .relay_urls()
    .map(ToString::to_string)
    .collect::<Vec<_>>();

  MemberRecord {
    endpoint_id: endpoint.id().to_string(),
    name: name.to_string(),
    protocols: protocols
      .iter()
      .map(|protocol| (*protocol).to_string())
      .collect(),
    blobs,
    addrs,
    relay_urls,
    updated_at: now_unix_secs(),
  }
}

pub(crate) fn provider_record(
  hash: Hash,
  format: BlobFormat,
  name: String,
  size: u64,
) -> BlobProviderRecord {
  BlobProviderRecord {
    hash: hash.to_string(),
    format: blob_format_name(format).to_string(),
    name,
    size,
  }
}
