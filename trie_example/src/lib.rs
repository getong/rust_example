use std::boxed::Box;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct IoTDevice {
  pub numerical_id: u64,
  pub path: String,
  pub address: String,
}

impl IoTDevice {
  pub fn new(id: u64, address: impl Into<String>, path: impl Into<String>) -> IoTDevice {
    IoTDevice {
      address: address.into(),
      numerical_id: id,
      path: path.into(),
    }
  }
}

impl PartialEq for IoTDevice {
  fn eq(&self, other: &IoTDevice) -> bool {
    self.numerical_id == other.numerical_id && self.address == other.address
  }
}

#[derive(Clone, Debug)]
pub struct MessageNotification {
  pub no_messages: u64,
  pub device: IoTDevice,
}

impl MessageNotification {
  pub fn new(device: IoTDevice, no_messages: u64) -> MessageNotification {
    MessageNotification {
      no_messages: no_messages,
      device: device,
    }
  }
}

impl PartialEq for MessageNotification {
  fn eq(&self, other: &MessageNotification) -> bool {
    self.device.eq(&other.device) && self.no_messages == other.no_messages
  }
}

type Link = Box<Node>;

struct Node {
  pub key: char,
  next: HashMap<char, Link>,
  pub value: Option<IoTDevice>,
}

impl Node {
  pub fn new(key: char, device: Option<IoTDevice>) -> Link {
    Box::new(Node {
      key: key,
      next: HashMap::new(),
      value: device,
    })
  }
}

impl PartialEq for Node {
  fn eq(&self, other: &Node) -> bool {
    self.key == other.key
  }
}

pub struct BestDeviceRegistry {
  pub length: u64,
  root: HashMap<char, Link>,
}

impl BestDeviceRegistry {
  pub fn new_empty() -> BestDeviceRegistry {
    BestDeviceRegistry {
      length: 0,
      root: HashMap::new(),
    }
  }

  pub fn add(&mut self, device: IoTDevice) {
    let p = device.path.clone();
    let mut path = p.chars();

    if let Some(start) = path.next() {
      self.length += 1;
      let mut n = self.root.entry(start).or_insert(Node::new(start, None));
      for c in path {
        let tmp = n.next.entry(c).or_insert(Node::new(c, None));
        n = tmp;
      }
      n.value = Some(device);
    }
  }

  pub fn find(&self, path: &str) -> Option<IoTDevice> {
    let mut path = path.chars();

    if let Some(start) = path.next() {
      self.root.get(&start).map_or(None, |mut n| {
        for c in path {
          match n.next.get(&c) {
            Some(ref tmp) => n = tmp,
            None => break,
          }
        }
        n.value.clone()
      })
    } else {
      None
    }
  }

  pub fn walk(&self, callback: impl Fn(&IoTDevice) -> ()) {
    for r in self.root.values() {
      self.walk_r(&r, &callback);
    }
  }

  fn walk_r(&self, node: &Link, callback: &impl Fn(&IoTDevice) -> ()) {
    for n in node.next.values() {
      self.walk_r(&n, callback);
    }
    if let Some(ref dev) = node.value {
      callback(dev);
    }
  }
}
