use alloy::{primitives::U256, providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  MultiSigWallet,
  "abi/MultiSigWallet.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestContract,
  "abi/TestContract.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(test_contract) = super::deployed_contract!(
    provider,
    TestContract,
    "TestContract.TestContract",
    "MultiSigWallet::TestContract"
  ) else {
    return Ok(());
  };
  let Some(wallet) = super::deployed_contract!(
    provider,
    MultiSigWallet,
    "MultiSigWallet.MultiSigWallet",
    "MultiSigWallet"
  ) else {
    return Ok(());
  };
  println!(
    "[MultiSigWallet] deployed: {}, helper TestContract: {}",
    wallet.address(),
    test_contract.address()
  );

  let tx_data = test_contract.getData().call().await?;
  wallet
    .submitTransaction(*test_contract.address(), U256::ZERO, tx_data)
    .send()
    .await?
    .watch()
    .await?;
  wallet
    .confirmTransaction(U256::ZERO)
    .send()
    .await?
    .watch()
    .await?;
  wallet
    .executeTransaction(U256::ZERO)
    .send()
    .await?
    .watch()
    .await?;

  let tx_count = wallet.getTransactionCount().call().await?;
  let helper_value = test_contract.i().call().await?;
  println!("[MultiSigWallet] tx_count={tx_count}, TestContract.i={helper_value}");
  Ok(())
}
