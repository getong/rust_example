use std::{future::Future, io, net::IpAddr, pin::Pin, str};

use anyhow::Result;
use hickory_resolver::{
  TokioAsyncResolver,
  config::{ResolverConfig, ResolverOpts},
};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};

#[tokio::main]
async fn main() {
  match resolve_libp2p_dnsaddr("bootstrap.libp2p.io").await {
    Ok(pairs) => {
      let tasks: Vec<_> = pairs
        .into_iter()
        .map(|(peer_id, addr)| {
          tokio::spawn(async move {
            match resolve_multiaddr_to_ip(addr).await {
              Ok(ip_addr) => println!("Peer ID: {}, Resolved IP Address: {}", peer_id, ip_addr),
              Err(e) => eprintln!("Failed to resolve IP for Peer ID {}: {:?}", peer_id, e),
            }
          })
        })
        .collect();

      for task in tasks {
        let _ = task.await;
      }
    }
    Err(e) => {
      eprintln!("Failed to resolve DNS: {:?}", e);
    }
  }
}

async fn resolve_libp2p_dnsaddr(domain: &str) -> Result<Vec<(PeerId, Multiaddr)>> {
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

/// Resolves the `dns` parts of a `Multiaddr` into an IP address.
fn resolve_multiaddr_to_ip(
  multiaddr: Multiaddr,
) -> Pin<Box<dyn Future<Output = Result<Multiaddr>> + Send>> {
  Box::pin(async move {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    let mut new_addr = Vec::new();
    for protocol in multiaddr.iter() {
      match protocol {
        Protocol::Dns4(domain) | Protocol::Dns6(domain) => {
          let response = resolver.lookup_ip(domain.to_string()).await?;
          if let Some(ip) = response.iter().next() {
            match ip {
              IpAddr::V4(ipv4) => new_addr.push(Protocol::Ip4(ipv4)),
              IpAddr::V6(ipv6) => new_addr.push(Protocol::Ip6(ipv6)),
            }
          } else {
            return Err(anyhow::anyhow!("No IP addresses found for {}", domain));
          }
        }
        Protocol::Dnsaddr(domain) => {
          let resolved_addrs = resolve_libp2p_dnsaddr(&domain).await?;
          if let Some((_, resolved_addr)) = resolved_addrs.into_iter().next() {
            return resolve_multiaddr_to_ip(resolved_addr).await; // Recursive call via Box::pin
          } else {
            return Err(anyhow::anyhow!("No resolved addresses for {}", domain));
          }
        }
        other => new_addr.push(other),
      }
    }
    Ok(Multiaddr::from_iter(new_addr))
  })
}

// copy from https://github.com/ChainSafe/forest/blob/main/src/libp2p/discovery.rs
// dig -t TXT _dnsaddr.bootstrap.libp2p.io
// copy from https://discuss.libp2p.io/t/how-to-interpret-multiaddr/1424/2
