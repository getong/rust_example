use std::{io, str};

use hickory_resolver::{
  TokioAsyncResolver,
  config::{ResolverConfig, ResolverOpts},
};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};

#[tokio::main]
async fn main() {
  match resolve_libp2p_dnsaddr("bootstrap.libp2p.io").await {
    Ok(pairs) => {
      for (peer_id, addr) in pairs {
        println!("Peer ID: {}, Address: {}", peer_id, addr);
      }
    }
    Err(e) => {
      eprintln!("Failed to resolve DNS: {:?}", e);
    }
  }
}

async fn resolve_libp2p_dnsaddr(domain: &str) -> anyhow::Result<Vec<(PeerId, Multiaddr)>> {
  let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
  let txt_records = resolver.txt_lookup(format!("_dnsaddr.{}", domain)).await?;

  let mut pairs = Vec::new();
  for txt in txt_records {
    for record in txt.txt_data() {
      if let Ok(addr) = parse_dnsaddr_txt(record) {
        if let Some(Protocol::P2p(peer_id)) = addr.iter().last() {
          pairs.push((peer_id, addr));
        } else {
          eprintln!("Failed to extract PeerId from address: {}", addr);
        }
      }
    }
  }
  Ok(pairs)
}

/// Parses a `<character-string>` of a `dnsaddr` `TXT` record.
fn parse_dnsaddr_txt(txt: &[u8]) -> io::Result<Multiaddr> {
  let s = str::from_utf8(txt).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
  if let Some(addr) = s.strip_prefix("dnsaddr=") {
    Multiaddr::try_from(addr).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
  } else {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Missing `dnsaddr=` prefix.",
    ))
  }
}

// copy from https://github.com/ChainSafe/forest/blob/main/src/libp2p/discovery.rs
// dig -t TXT _dnsaddr.bootstrap.libp2p.io
// copy from dig -t TXT _dnsaddr.bootstrap.libp2p.io