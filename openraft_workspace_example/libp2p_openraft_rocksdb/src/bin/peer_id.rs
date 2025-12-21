use std::path::PathBuf;

use anyhow::{anyhow, Context};
use clap::Parser;
use libp2p::{identity, PeerId};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Print libp2p PeerId from a protobuf key file")]
struct Opt {
  /// Path to the protobuf-encoded libp2p keypair (same format used by the example).
  #[arg(long)]
  key: PathBuf,

  /// Create the key file if it does not exist.
  #[arg(long, default_value_t = false)]
  create: bool,
}

fn load_or_create(opt: &Opt) -> anyhow::Result<identity::Keypair> {
  if let Ok(bytes) = std::fs::read(&opt.key) {
    return identity::Keypair::from_protobuf_encoding(&bytes)
      .map_err(|e| anyhow!("invalid key file: {e}"));
  }

  if !opt.create {
    return Err(anyhow!(
      "key file not found: {} (pass --create to generate)",
      opt.key.display()
    ));
  }

  if let Some(parent) = opt.key.parent() {
    std::fs::create_dir_all(parent).ok();
  }

  let kp = identity::Keypair::generate_ed25519();
  let bytes = kp
    .to_protobuf_encoding()
    .map_err(|e| anyhow!("failed to encode keypair: {e}"))?;
  std::fs::write(&opt.key, bytes).context("write keypair")?;
  Ok(kp)
}

fn main() -> anyhow::Result<()> {
  let opt = Opt::parse();
  let kp = load_or_create(&opt)?;
  let peer_id = PeerId::from(kp.public());
  println!("{peer_id}");
  Ok(())
}
