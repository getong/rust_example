use std::{
  net::{IpAddr, SocketAddr},
  str::FromStr,
};

use hashring::HashRing;

#[derive(Debug, Copy, Clone, Hash, PartialEq)]
struct VNode {
  id: usize,
  addr: SocketAddr,
}

impl VNode {
  fn new(ip: &str, port: u16, id: usize) -> Self {
    let addr = SocketAddr::new(IpAddr::from_str(&ip).unwrap(), port);
    VNode { id: id, addr: addr }
  }
}

impl ToString for VNode {
  fn to_string(&self) -> String {
    format!("{}|{}", self.addr, self.id)
  }
}

fn main() {
  let mut ring: HashRing<VNode> = HashRing::new();

  let mut nodes = vec![];
  nodes.push(VNode::new("127.0.0.1", 1024, 1));
  nodes.push(VNode::new("127.0.0.1", 1024, 2));
  nodes.push(VNode::new("127.0.0.2", 1024, 1));
  nodes.push(VNode::new("127.0.0.2", 1024, 2));
  nodes.push(VNode::new("127.0.0.2", 1024, 3));
  nodes.push(VNode::new("127.0.0.3", 1024, 1));

  for node in nodes {
    ring.add(node);
  }

  println!("{:?}", ring.get(&"foo"));
  println!("{:?}", ring.get(&"bar"));
  println!("{:?}", ring.get(&"baz"));
}
