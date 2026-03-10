use alloy::{
  primitives::{Address, U256},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  AbiEncode,
  "abi/AbiEncode.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Token,
  "abi/Token.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let token = Token::deploy(provider).await?;
  let contract = AbiEncode::deploy(provider).await?;
  println!(
    "[AbiEncode] deployed: {}, helper Token: {}",
    contract.address(),
    token.address()
  );

  let to = Address::repeat_byte(0x11);
  let sig_data = contract
    .encodeWithSignature(to, U256::from(9_u64))
    .call()
    .await?;
  let selector_data = contract
    .encodeWithSelector(to, U256::from(9_u64))
    .call()
    .await?;
  let call_data = contract.encodeCall(to, U256::from(9_u64)).call().await?;

  contract
    .test(*token.address(), call_data.clone())
    .send()
    .await?
    .watch()
    .await?;

  println!(
    "[AbiEncode] signature_len={}, selector_len={}, call_len={}",
    sig_data.len(),
    selector_data.len(),
    call_data.len()
  );
  Ok(())
}
