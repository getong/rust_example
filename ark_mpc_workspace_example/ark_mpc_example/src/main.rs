use ark_mpc::{
  algebra::Scalar,
  test_helpers::{execute_mock_mpc, TestCurve},
  MpcFabric, PARTY0, PARTY1,
};
use rand::thread_rng;

type Curve = TestCurve;

#[tokio::main]
async fn main() {
  let (party0_res, party1_res) = execute_mock_mpc(|fabric: MpcFabric<Curve>| async move {
    let my_val = {
      let mut rng = thread_rng();
      Scalar::<Curve>::random(&mut rng)
    };

    let a = fabric.share_scalar(my_val, PARTY0 /* sender */); // party0 value
    let b = fabric.share_scalar(my_val, PARTY1 /* sender */); // party1 value
    let c = a * b;

    c.open_authenticated().await.expect("authentication error")
  })
  .await;

  assert_eq!(party0_res, party1_res);
  println!("a * b = {party0_res}");
}
