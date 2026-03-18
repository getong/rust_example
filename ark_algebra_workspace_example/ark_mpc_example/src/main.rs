use std::env;

use ark_curve25519::EdwardsProjective as Curve25519Projective;
use ark_mpc::{
  MpcFabric, PARTY0, PARTY1, algebra::Scalar, network::QuicTwoPartyNet,
  offline_prep::PartyIDBeaverSource,
};
use rand::rng;

type Curve = Curve25519Projective;

#[tokio::main]
async fn main() {
  rustls::crypto::ring::default_provider()
    .install_default()
    .expect("Failed to install rustls crypto provider");

  let args: Vec<String> = env::args().collect();
  let party: u64 = args
    .get(1)
    .expect("Usage: ark_mpc_example <0|1>")
    .parse()
    .expect("Party must be 0 or 1");

  let (party_id, local_addr, peer_addr) = if party == 0 {
    (PARTY0, "127.0.0.1:8000", "127.0.0.1:9000")
  } else {
    (PARTY1, "127.0.0.1:9000", "127.0.0.1:8000")
  };

  let mut network = QuicTwoPartyNet::new(
    party_id,
    local_addr.parse().unwrap(),
    peer_addr.parse().unwrap(),
  );
  network
    .connect()
    .await
    .expect("failed to connect MPC network");

  let beaver = PartyIDBeaverSource::new(party_id);

  let mut rng = rng();
  let my_val = Scalar::<Curve>::random(&mut rng);
  let fabric = MpcFabric::new(network, beaver);

  let a = fabric.share_scalar(my_val, PARTY0); // party0 value
  let b = fabric.share_scalar(my_val, PARTY1); // party1 value
  let c = a * b;

  let res = c.open_authenticated().await.expect("authentication error");
  println!("Party {party}: a * b = {res}");
}

// cargo run -- 1
// cargo run -- 0
