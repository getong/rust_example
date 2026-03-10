use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestContract,
  "abi/TestContract.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let contract = TestContract::deploy(provider).await?;
  println!("[TestContract] deployed: {}", contract.address());

  contract
    .callMe(U256::from(7_u64))
    .send()
    .await?
    .watch()
    .await?;
  let current = contract.i().call().await?;
  let calldata = contract.getData().call().await?;
  println!(
    "[TestContract] i={current}, encoded_call_len={}",
    calldata.len()
  );
  Ok(())
}
