use std::{net::ToSocketAddrs, str::FromStr};

use libp2p::multiaddr::{Multiaddr, Protocol};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let multiaddr_str1 = "/dns4/boot_node/tcp/8002";
  let multiaddr_str2 = "/dns4/boot_node/udp/8003/quic-v1";

  match parse_and_resolve_multiaddr(multiaddr_str1)? {
    resolved_multiaddrs => {
      for addr in resolved_multiaddrs {
        println!("Final Resolved Multiaddr: {:?}", addr);
      }
    }
  }

  match parse_and_resolve_multiaddr(multiaddr_str2)? {
    resolved_multiaddrs => {
      for addr in resolved_multiaddrs {
        println!("Final Resolved Multiaddr: {:?}", addr);
      }
    }
  }

  Ok(())
}

fn parse_and_resolve_multiaddr(
  multiaddr_str: &str,
) -> Result<Vec<Multiaddr>, Box<dyn std::error::Error>> {
  println!("Parsed multiaddress: {:?}", multiaddr_str);
  let multiaddr = Multiaddr::from_str(multiaddr_str)?;

  let mut dns_name = None;
  let mut port = None;
  let mut transport_protocols = Vec::new();

  for protocol in multiaddr.iter() {
    match protocol {
      Protocol::Dns4(name) => dns_name = Some(name),
      Protocol::Tcp(p) | Protocol::Udp(p) => {
        port = Some(p);
        transport_protocols.push(protocol.clone()); // Preserve TCP/UDP protocol
      }
      _ => transport_protocols.push(protocol.clone()), /* Preserve additional protocols (e.g.,
                                                        * quic-v1) */
    }
  }

  let mut resolved_multiaddrs = Vec::new();

  if let (Some(dns_name), Some(port)) = (dns_name, port) {
    let addr = format!("{}:{}", dns_name, port);
    if let Ok(resolved) = addr.to_socket_addrs() {
      for resolved_ip in resolved {
        let mut resolved_multiaddr = Multiaddr::empty();
        resolved_multiaddr.push(Protocol::Ip4(resolved_ip.ip().to_string().parse()?));

        // Append the transport protocol and port
        for protocol in &transport_protocols {
          resolved_multiaddr.push(protocol.clone());
        }

        resolved_multiaddrs.push(resolved_multiaddr);
      }
    }
  }

  if resolved_multiaddrs.is_empty() {
    Err("Failed to resolve DNS or extract the required data".into())
  } else {
    Ok(resolved_multiaddrs)
  }
}

// in /etc/hosts file
// 192.168.1.136   boot_node
