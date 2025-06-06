extern crate trust_dns_client;
extern crate trust_dns_resolver;

use std::env;

use trust_dns_client::rr::record_type::RecordType;
use trust_dns_resolver::{config::*, Resolver};

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() != 2 {
    eprintln!("Please provide a name to query");
    std::process::exit(1);
  }

  let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default()).unwrap();
  // Add a dot to the given name
  let query = format!("{}.", args[1]);

  // Run the DNS query
  let response = resolver.lookup_ip(query.as_str());
  println!("Using the synchronous resolver");
  for ans in response.iter() {
    println!("{:?}", ans);
  }

  println!("Using the system resolver");
  let system_resolver = Resolver::from_system_conf().unwrap();
  let system_response = system_resolver.lookup_ip(query.as_str());
  for ans in system_response.iter() {
    println!("{:?}", ans);
  }

  let ns = resolver.lookup(query.as_str(), RecordType::NS);
  println!("NS records using the synchronous resolver");
  for ans in ns.iter() {
    println!("{:?}", ans);
  }
}
