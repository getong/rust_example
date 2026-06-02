use ark_mpc::{
  MpcFabric, PARTY0, PARTY1,
  algebra::{AuthenticatedScalarResult, Scalar},
  test_helpers::{TestCurve, execute_mock_mpc},
};
use futures::future::join_all;

pub mod threshold_certificate;

pub type Curve = TestCurve;
pub type MpcScalar = Scalar<Curve>;
pub type AuthenticatedScalar = AuthenticatedScalarResult<Curve>;
pub type Fabric = MpcFabric<Curve>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniOrder {
  pub base_amount: MpcScalar,
  pub price: MpcScalar,
}

impl MiniOrder {
  pub fn new(base_amount: u64, price: u64) -> Self {
    Self {
      base_amount: MpcScalar::from(base_amount),
      price: MpcScalar::from(price),
    }
  }

  fn to_scalars(self) -> Vec<MpcScalar> {
    vec![self.base_amount, self.price]
  }

  fn from_scalars(values: Vec<MpcScalar>) -> Self {
    assert_eq!(values.len(), 2);
    Self {
      base_amount: values[0],
      price: values[1],
    }
  }

  pub fn allocate(self, sender: u64, fabric: &Fabric) -> AuthenticatedMiniOrder {
    let mut values = fabric
      .batch_share_scalar(self.to_scalars(), sender)
      .into_iter();
    AuthenticatedMiniOrder {
      base_amount: values.next().unwrap(),
      price: values.next().unwrap(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedMiniOrder {
  pub base_amount: AuthenticatedScalar,
  pub price: AuthenticatedScalar,
}

impl AuthenticatedMiniOrder {
  fn to_authenticated_scalars(&self) -> Vec<AuthenticatedScalar> {
    vec![self.base_amount.clone(), self.price.clone()]
  }

  pub async fn open_and_authenticate(&self) -> MiniOrder {
    let values = AuthenticatedScalar::open_authenticated_batch(&self.to_authenticated_scalars());
    let opened = join_all(values)
      .await
      .into_iter()
      .collect::<Result<Vec<_>, _>>()
      .expect("authentication error");

    MiniOrder::from_scalars(opened)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiniOrderComputation {
  pub filled_base: MpcScalar,
  pub quote_total: MpcScalar,
  pub remaining_base: MpcScalar,
}

impl MiniOrderComputation {
  fn from_scalars(values: Vec<MpcScalar>) -> Self {
    assert_eq!(values.len(), 3);
    Self {
      filled_base: values[0],
      quote_total: values[1],
      remaining_base: values[2],
    }
  }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedMiniOrderComputation {
  pub filled_base: AuthenticatedScalar,
  pub quote_total: AuthenticatedScalar,
  pub remaining_base: AuthenticatedScalar,
}

impl AuthenticatedMiniOrderComputation {
  fn to_authenticated_scalars(&self) -> Vec<AuthenticatedScalar> {
    vec![
      self.filled_base.clone(),
      self.quote_total.clone(),
      self.remaining_base.clone(),
    ]
  }

  pub async fn open_and_authenticate(&self) -> MiniOrderComputation {
    let values = AuthenticatedScalar::open_authenticated_batch(&self.to_authenticated_scalars());
    let opened = join_all(values)
      .await
      .into_iter()
      .collect::<Result<Vec<_>, _>>()
      .expect("authentication error");

    MiniOrderComputation::from_scalars(opened)
  }
}

pub fn compute_mini_order(
  order: &AuthenticatedMiniOrder,
  fill_amount: &AuthenticatedScalar,
) -> AuthenticatedMiniOrderComputation {
  AuthenticatedMiniOrderComputation {
    filled_base: fill_amount.clone(),
    quote_total: fill_amount * &order.price,
    remaining_base: &order.base_amount - fill_amount,
  }
}

pub async fn run_scalar_product_example() -> MpcScalar {
  let party0_value = MpcScalar::from(7u64);
  let party1_value = MpcScalar::from(11u64);
  let expected = party0_value * party1_value;

  let (party0_res, party1_res) = execute_mock_mpc(move |fabric: Fabric| async move {
    let a = fabric.share_scalar(party0_value, PARTY0);
    let b = fabric.share_scalar(party1_value, PARTY1);
    let c = a * b;

    c.open_authenticated().await.expect("authentication error")
  })
  .await;

  assert_eq!(party0_res, expected);
  assert_eq!(party1_res, expected);
  party0_res
}

pub async fn run_mini_order_example() -> MiniOrderComputation {
  let party0_order = MiniOrder::new(100, 3);
  let party1_fill_amount = MpcScalar::from(40u64);

  let expected = MiniOrderComputation {
    filled_base: party1_fill_amount,
    quote_total: MpcScalar::from(120u64),
    remaining_base: MpcScalar::from(60u64),
  };

  let (party0_res, party1_res) = execute_mock_mpc(move |fabric: Fabric| async move {
    let order = party0_order.allocate(PARTY0, &fabric);
    let fill_amount = fabric.share_scalar(party1_fill_amount, PARTY1);
    let result = compute_mini_order(&order, &fill_amount);

    result.open_and_authenticate().await
  })
  .await;

  assert_eq!(party0_res, expected);
  assert_eq!(party1_res, expected);
  party0_res
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn scalar_product_matches_plaintext_model() {
    assert_eq!(run_scalar_product_example().await, MpcScalar::from(77u64));
  }

  #[tokio::test]
  async fn mini_order_matches_plaintext_model() {
    let result = run_mini_order_example().await;

    assert_eq!(
      result,
      MiniOrderComputation {
        filled_base: MpcScalar::from(40u64),
        quote_total: MpcScalar::from(120u64),
        remaining_base: MpcScalar::from(60u64),
      }
    );
  }

  #[tokio::test]
  async fn mini_order_open_round_trip() {
    let order = MiniOrder::new(12, 9);

    let (party0_res, party1_res) = execute_mock_mpc(move |fabric: Fabric| async move {
      order
        .allocate(PARTY0, &fabric)
        .open_and_authenticate()
        .await
    })
    .await;

    assert_eq!(party0_res, order);
    assert_eq!(party1_res, order);
  }
}
