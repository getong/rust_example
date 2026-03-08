use alloy::{
  primitives::U256,
  providers::{Provider, ProviderBuilder},
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

#[tokio::main]
async fn main() -> Result<()> {
  // Start a local Anvil node and connect with the default funded wallet.
  let provider = ProviderBuilder::new().connect_anvil_with_wallet();

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
