use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Account,
  "abi/Account.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Account, "Error.Account", "Error.sol::Account")
  else {
    return Ok(());
  };

  contract
    .deposit(U256::from(200_u64))
    .send()
    .await?
    .watch()
    .await?;
  contract
    .withdraw(U256::from(80_u64))
    .send()
    .await?
    .watch()
    .await?;

  let balance = contract.balance().call().await?;
  println!("[Error.sol::Account] balance = {balance}");
  Ok(())
}
