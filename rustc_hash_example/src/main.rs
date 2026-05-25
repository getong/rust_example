use std::hash::{BuildHasher, Hash};

use rustc_hash::FxBuildHasher;

fn hash_u64<T: Hash>(value: &T) -> u64 {
  FxBuildHasher.hash_one(value)
}

#[derive(Hash)]
struct WrappedPeerId(String);

impl WrappedPeerId {
  fn from_public_key_bytes(bytes: &[u8]) -> Self {
    Self(bs58::encode(bytes).into_string())
  }
}

fn main() {
  let peer = WrappedPeerId::from_public_key_bytes(b"dummy_public_key_32_bytes____");
  let raft_id = hash_u64(&peer);

  println!("PeerId:  {}", peer.0);
  println!("RaftId:  {raft_id:#018x}");
  println!("RaftId:  {raft_id}");
}
