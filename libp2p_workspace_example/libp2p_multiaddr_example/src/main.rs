use libp2p::{Multiaddr, multiaddr::Protocol};

#[tokio::main]
async fn main() {
  test_multiaddr();
}

fn test_multiaddr() {
  let addr: Multiaddr = "/dns/example.com/tcp/8080".parse().unwrap();
  assert_eq!(addr.to_string(), "/dns/example.com/tcp/8080");
  assert!(!is_http(&addr));
  assert!(!is_tls(&addr));

  let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
  assert_eq!(addr.to_string(), "/ip4/127.0.0.1/tcp/8080");
  assert!(!is_http(&addr));
  assert!(!is_tls(&addr));

  let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/tls".parse().unwrap();
  assert_eq!(addr.to_string(), "/ip4/127.0.0.1/tcp/8080/tls");
  assert!(!is_http(&addr));
  assert!(is_tls(&addr));

  let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080/http".parse().unwrap();
  assert_eq!(addr.to_string(), "/ip4/127.0.0.1/tcp/8080/http");
  assert!(is_http(&addr));
  assert!(!is_tls(&addr));

  let addr: Multiaddr = "/ip6/::/tcp/8080/https".parse().unwrap();
  assert_eq!(addr.to_string(), "/ip6/::/tcp/8080/https");
  assert!(!is_http(&addr));
  assert!(is_tls(&addr));

  let addr: Multiaddr = "/ip4/127.0.0.1/udp/8080".parse().unwrap();
  assert_eq!(addr.to_string(), "/ip4/127.0.0.1/udp/8080");
  assert!(!is_http(&addr));
  assert!(!is_tls(&addr));
  assert!(is_udp(&addr));
}

fn is_http(addr: &Multiaddr) -> bool {
  addr.iter().any(|p| matches!(p, Protocol::Http))
}

fn is_tls(addr: &Multiaddr) -> bool {
  addr
    .iter()
    .any(|p| matches!(p, Protocol::Tls) || matches!(p, Protocol::Https))
}

fn is_udp(addr: &Multiaddr) -> bool {
  addr.iter().any(|p| matches!(p, Protocol::Udp(_)))
}
