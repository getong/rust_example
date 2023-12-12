pub enum Protection {
  Secure { version: u64 },
  Insecure,
}

fn process(prot: Protection) {
  match prot {
    Protection::Secure { version } => {
      println!("Hacker-safe thanks to protocol v{version}");
    }
    Protection::Insecure => {
      println!("Come on in");
    }
  }
}

fn main() {
  process(Protection::Secure { version: 2 })
}
