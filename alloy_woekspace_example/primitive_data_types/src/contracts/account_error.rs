use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Error,
  "abi/AccountError.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = Error::deploy(provider).await?;
  println!("[Account.sol::Error] deployed: {}", contract.address());

  contract.testRequire(U256::from(11_u64)).call().await?;
  contract.testRevert(U256::from(11_u64)).call().await?;
  contract.testAssert().call().await?;
  contract.testCustomError(U256::ZERO).call().await?;

  let num = contract.num().call().await?;
  println!("[Account.sol::Error] num = {num}");
  Ok(())
}
