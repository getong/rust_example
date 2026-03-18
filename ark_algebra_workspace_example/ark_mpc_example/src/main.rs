use ark_curve25519::EdwardsProjective as Curve25519Projective;
use ark_mpc::{
  MpcFabric, PARTY0, PARTY1, algebra::scalar::Scalar, beaver::PreprocessingPhase,
  network::QuicTwoPartyNet,
};
use rand::thread_rng;

type Curve = Curve25519Projective;

#[tokio::main]
async fn main() {
  // Beaver source should be defined outside of the crate and rely on separate infrastructure
  let beaver = BeaverSource::new();

  let local_addr = "127.0.0.1:8000".parse().unwrap();
  let peer_addr = "127.0.0.1:9000".parse().unwrap();
  let network = QuicTwoPartyNet::new(PARTY0, local_addr, peer_addr);

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
