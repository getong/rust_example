use alloy::{
  network::EthereumWallet,
  node_bindings::Anvil,
  primitives::U256,
  providers::{Provider, ProviderBuilder},
  signers::local::PrivateKeySigner,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Counter,
  "abi/Counter.json"
);

#[tokio::main]
async fn main() -> Result<()> {
  // Launch a local Anvil chain and use the first default account as signer.
  let anvil = Anvil::new().try_spawn()?;
  let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
  let wallet = EthereumWallet::from(signer);

  let provider = ProviderBuilder::new()
    .wallet(wallet)
    .connect_http(anvil.endpoint_url());

  let chain_id = provider.get_chain_id().await?;
  println!("Connected chain id: {chain_id}");

  // Same constructor as Solidity: `constructor(uint256 initialNumber)`.
  let contract = Counter::deploy(&provider, U256::from(11_u64)).await?;
  println!("Counter deployed at: {}", contract.address());

  let number_after_deploy = contract.number().call().await?;
  println!("number after deploy: {number_after_deploy}");

  contract
    .setNumber(U256::from(43_u64))
    .send()
    .await?
    .watch()
    .await?;
  let number_after_set = contract.number().call().await?;
  println!("number after setNumber(43): {number_after_set}");

  contract.increment().send().await?.watch().await?;
  let number_after_increment = contract.number().call().await?;
  println!("number after increment(): {number_after_increment}");

  contract
    .resetToNetworkDefault()
    .send()
    .await?
    .watch()
    .await?;
  let number_after_reset = contract.number().call().await?;
  println!("number after resetToNetworkDefault(): {number_after_reset}");

  // In config_example, chain id 31337 (Anvil) maps to initialNumber = 7.
  assert_eq!(number_after_reset, U256::from(7_u64));
  println!("OK: reset value matches network config (7 on Anvil)");

  Ok(())
}
