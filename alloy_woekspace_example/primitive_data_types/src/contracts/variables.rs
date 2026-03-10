use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Variables,
  "abi/Variables.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) =
    super::deployed_contract!(provider, Variables, "State.Variables", "Variables")
  else {
    return Ok(());
  };

  contract.doSomething().send().await?.watch().await?;

  let text = contract.text().call().await?;
  let num = contract.num().call().await?;
  println!("[Variables] text={text}, num={num}");
  Ok(())
}
