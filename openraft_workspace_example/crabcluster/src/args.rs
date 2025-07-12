use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "crabcluster", author, version, about)]
pub struct Args {
  #[arg(long)]
  pub bind_addr: SocketAddr,
}
