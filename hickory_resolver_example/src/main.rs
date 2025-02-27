use std::io;

use hickory_resolver::{
  TokioAsyncResolver,
  config::{ResolverConfig, ResolverOpts},
};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};

#[tokio::main]
async fn main() {
  if let Ok(pairs) = resolve_libp2p_dnsaddr("/dns4/bootstrap.libp2p.io/tcp/8080").await {
    for (peer_id, addr) in pairs {
      println!("Peer ID: {}, Address: {}", peer_id, addr);
    }
  }
}

async fn resolve_libp2p_dnsaddr(name: &str) -> anyhow::Result<Vec<(PeerId, Multiaddr)>> {
  let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
  // let name = ["_dnsaddr.", name].concat();
  let txts = resolver.txt_lookup(name).await?;

  let mut pairs = vec![];
  for txt in txts {
    match txt.txt_data().first() {
      Some(chars) => match parse_dnsaddr_txt(chars) {
        Err(e) => {
          // Skip over seemingly invalid entries.
          println!("Invalid TXT record: {:?}", e);
        }
        Ok(mut addr) => {
          if let Some(Protocol::P2p(peer_id)) = addr.pop() {
            pairs.push((peer_id, addr))
          } else {
            println!("Failed to parse peer id from {addr}")
          }
        }
      },
      None => {
        println!(" txt is {}, none", txt);
      }
    }
  }
  Ok(pairs)
}

/// Parses a `<character-string>` of a `dnsaddr` `TXT` record.
fn parse_dnsaddr_txt(txt: &[u8]) -> io::Result<Multiaddr> {
  let s = str::from_utf8(txt).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
  match s.strip_prefix("dnsaddr=") {
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Missing `dnsaddr=` prefix.",
    )),
    Some(a) => {
      Ok(Multiaddr::try_from(a).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)
    }
  }
}

// copy from https://github.com/ChainSafe/forest/blob/main/src/libp2p/discovery.rs
