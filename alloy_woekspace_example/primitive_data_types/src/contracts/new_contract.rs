use alloy::{
  primitives::{B256, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  CarFactory,
  "abi/CarFactory.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let owner = provider
    .get_accounts()
    .await?
    .first()
    .copied()
    .ok_or_else(|| eyre::eyre!("no unlocked account available from provider"))?;

  let contract = CarFactory::deploy(provider).await?;
  println!("[NewContract] CarFactory deployed: {}", contract.address());

  contract
    .create(owner, "Tesla".to_string())
    .send()
    .await?
    .watch()
    .await?;
  contract
    .create2(owner, "BMW".to_string(), B256::with_last_byte(1))
    .send()
    .await?
    .watch()
    .await?;

  let car0 = contract.getCar(U256::ZERO).call().await?;
  let car1 = contract.getCar(U256::from(1_u64)).call().await?;
  println!(
    "[NewContract] car0=({}, {}, {}, {}), car1=({}, {}, {}, {})",
    car0.owner,
    car0.model,
    car0.carAddr,
    car0.balance,
    car1.owner,
    car1.model,
    car1.carAddr,
    car1.balance
  );
  Ok(())
}
