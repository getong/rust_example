use ark_curve25519::EdwardsProjective as Curve25519Projective;
use ark_mpc::{
  MpcFabric, PARTY0, PARTY1,
  algebra::Scalar,
  network::QuicTwoPartyNet,
  offline_prep::PartyIDBeaverSource,
};
use rand::thread_rng;

type Curve = Curve25519Projective;

#[tokio::main]
async fn main() {
  let local_addr = "127.0.0.1:8000".parse().unwrap();
  let peer_addr = "127.0.0.1:9000".parse().unwrap();
  let mut network = QuicTwoPartyNet::new(PARTY0, local_addr, peer_addr);
  network.connect().await.expect("failed to connect MPC network");
  // Demo-only offline source; production should use a real preprocessing service.
  let beaver = PartyIDBeaverSource::new(PARTY0);

  // MPC circuit
  let mut rng = thread_rng();
  let my_val = Scalar::<Curve>::random(&mut rng);
  let fabric = MpcFabric::new(network, beaver);

  let a = fabric.share_scalar(my_val, PARTY0 /* sender */); // party0 value
  let b = fabric.share_scalar(my_val, PARTY1 /* sender */); // party1 value
  let c = a * b;

  let res = c.open_authenticated().await.expect("authentication error");
  println!("a * b = {res}");
}
