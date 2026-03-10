use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  SimpleStorage,
  "abi/SimpleStorage.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, SimpleStorage, "SimpleStorage", "SimpleStorage")
  else {
    return Ok(());
  };

  contract
    .set(U256::from(66_u64))
    .send()
    .await?
    .watch()
    .await?;

  let get_val = contract.get().call().await?;
  let num = contract.num().call().await?;
  println!("[SimpleStorage] get()={get_val}, num={num}");
  Ok(())
}
