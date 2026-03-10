use alloy::{
  primitives::{Address, U256},
  providers::Provider,
  sol,
};
use eyre::{Result, ensure};

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  HashFunction,
  "abi/HashFunction.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  GuessTheMagicWord,
  "abi/GuessTheMagicWord.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let hash_contract = HashFunction::deploy(provider).await?;
  let guess_contract = GuessTheMagicWord::deploy(provider).await?;
  println!(
    "[Keccak256] HashFunction: {}, GuessTheMagicWord: {}",
    hash_contract.address(),
    guess_contract.address()
  );

  let hash = hash_contract
    .hash(
      "hello".to_string(),
      U256::from(123_u64),
      Address::repeat_byte(0x11),
    )
    .call()
    .await?;
  let collision = hash_contract
    .collision("AA".to_string(), "ABBB".to_string())
    .call()
    .await?;
  let guessed = guess_contract.guess("Solidity".to_string()).call().await?;
  ensure!(
    guessed,
    "expected Solidity to match GuessTheMagicWord.answer"
  );
  println!("[Keccak256] hash={hash}, collision={collision}, guessed={guessed}");
  Ok(())
}
