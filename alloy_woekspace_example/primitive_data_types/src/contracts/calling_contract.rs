use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Callee,
  "abi/Callee.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  CallingContractCaller,
  "abi/CallingContractCaller.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let callee = Callee::deploy(provider).await?;
  let caller = CallingContractCaller::deploy(provider).await?;
  println!(
    "[CallingContract] caller: {}, callee: {}",
    caller.address(),
    callee.address()
  );

  caller
    .setX(*callee.address(), U256::from(11_u64))
    .send()
    .await?
    .watch()
    .await?;
  caller
    .setXFromAddress(*callee.address(), U256::from(22_u64))
    .send()
    .await?
    .watch()
    .await?;
  caller
    .setXandSendEther(*callee.address(), U256::from(33_u64))
    .value(U256::from(1_u64))
    .send()
    .await?
    .watch()
    .await?;

  let x = callee.x().call().await?;
  let value = callee.value().call().await?;
  println!("[CallingContract] callee.x={x}, callee.value={value}");
  Ok(())
}
