use std::env;

use alloy::{
  network::EthereumWallet,
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

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Dog,
  "abi/Dog.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Shepherd,
  "abi/Shepherd.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  HuskyTracker,
  "abi/HuskyTracker.json"
);

const DEFAULT_ANVIL_RPC: &str = "http://127.0.0.1:8545";
const DEFAULT_ANVIL_PRIVATE_KEY: &str =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

#[tokio::main]
async fn main() -> Result<()> {
  let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| DEFAULT_ANVIL_RPC.to_string());
  let private_key = env::var("PRIVATE_KEY").unwrap_or_else(|_| {
    println!(
      "PRIVATE_KEY not set, fallback to default Anvil key. This only works for local Anvil-like \
       nodes."
    );
    DEFAULT_ANVIL_PRIVATE_KEY.to_string()
  });

  if env::var("PRIVATE_KEY").is_err() && !is_local_endpoint(&rpc_url) {
    eyre::bail!(
      "PRIVATE_KEY is required for non-local RPC endpoints. Set both RPC_URL and PRIVATE_KEY."
    );
  }

  let signer: PrivateKeySigner = private_key.parse()?;
  let wallet = EthereumWallet::from(signer);

  let provider = ProviderBuilder::new()
    .wallet(wallet)
    .connect_http(rpc_url.parse()?);

  println!("connected rpc url: {rpc_url}");

  let chain_id = provider.get_chain_id().await?;
  println!("connected chain id: {chain_id}");

  let dog = Dog::deploy(&provider).await?;
  let shepherd = Shepherd::deploy(&provider).await?;
  let husky = HuskyTracker::deploy(&provider).await?;
  let counter = Counter::deploy(&provider).await?;

  println!("dog      : {}", dog.address());
  println!("shepherd : {}", shepherd.address());
  println!("husky    : {}", husky.address());
  println!("counter  : {}", counter.address());

  let dog_species = dog.species().call().await?;
  let dog_sound = dog.makeSound().call().await?;
  println!("dog.species() = {dog_species}");
  println!("dog.makeSound() = {dog_sound}");

  let dog_info = counter.readDogInfo(*dog.address()).call().await?;
  println!(
    "counter.readDogInfo() -> species={}, sound={}",
    dog_info.species, dog_info.sound
  );

  counter
    .setNumber(U256::from(7_u64))
    .send()
    .await?
    .watch()
    .await?;
  counter.increment().send().await?.watch().await?;
  let number_after_increment = counter.number().call().await?;
  println!("counter.number after set+increment = {number_after_increment}");

  counter
    .syncNumberFromShepherd(*shepherd.address())
    .send()
    .await?
    .watch()
    .await?;
  let number_after_shepherd = counter.number().call().await?;
  println!("counter.number after syncNumberFromShepherd = {number_after_shepherd}");

  counter
    .syncNumberFromHusky(*husky.address())
    .send()
    .await?
    .watch()
    .await?;
  let number_after_husky = counter.number().call().await?;
  println!("counter.number after syncNumberFromHusky = {number_after_husky}");

  assert_eq!(number_after_shepherd, U256::from(120_u64));
  assert_eq!(
    number_after_husky,
    U256::from("target locked by husky".len())
  );
  println!("all checks passed");

  Ok(())
}

fn is_local_endpoint(rpc_url: &str) -> bool {
  rpc_url.contains("127.0.0.1") || rpc_url.contains("localhost")
}
