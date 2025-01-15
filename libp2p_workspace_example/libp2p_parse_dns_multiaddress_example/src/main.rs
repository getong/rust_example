use std::{net::ToSocketAddrs, str::FromStr};

use libp2p::multiaddr::{Multiaddr, Protocol};

// in /etc/hosts file
// 192.168.1.136   boot_node

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // DNS-based multiaddress
  let multiaddr_str = "/dns4/boot_node/tcp/8002";

  // Parse the Multiaddress
  let multiaddr = Multiaddr::from_str(multiaddr_str)?;
  println!("Parsed multiaddress: {:?}", multiaddr);

  // Extract the DNS name and port
  let mut dns_name = None;
  let mut port = None;

  for protocol in multiaddr.iter() {
    match protocol {
      Protocol::Dns4(name) => dns_name = Some(name),
      Protocol::Tcp(p) => port = Some(p),
      _ => {}
    }
  }

  if let (Some(dns_name), Some(port)) = (dns_name, port) {
    // Resolve DNS to IP
    let addr = format!("{}:{}", dns_name, port);
    let resolved = addr.to_socket_addrs()?;

    for resolved_ip in resolved {
      println!("Resolved IP address: {}", resolved_ip);
      // Construct a new Multiaddr with the resolved IP
      let resolved_multiaddr =
        Multiaddr::from_str(&format!("/ip4/{}/tcp/{}", resolved_ip.ip(), port))?;
      println!("Resolved Multiaddr: {:?}", resolved_multiaddr);
    }
  } else {
    println!("Failed to extract DNS name or port from the multiaddress");
  }

  Ok(())
}
