use alloy::{
  primitives::{U256, address, bytes},
  providers::Provider,
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  VerifySignature,
  "abi/VerifySignature.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    VerifySignature,
    "VerifySignature",
    "VerifySignature"
  ) else {
    return Ok(());
  };

  let signer = address!("0xB273216C05A8c0D4F0a4Dd0d7Bae1D2EfFE636dd");
  let to = address!("0x14723A09ACff6D2A60DcdF7aA4AFf308FDDC160C");
  let amount = U256::from(123_u64);
  let nonce = U256::from(1_u64);
  let message = "coffee and donuts".to_string();
  let signature = bytes!(
    "0x993dab3dd91f5c6dc28e17439be475478f5635c92a56e17e82349d3fb2f166196f466c0b4e0c146f285204f0dcb13e5ae67bc33f4b888ec32dfe0a063e8f3f781b"
  );

  let message_hash = contract
    .getMessageHash(to, amount, message.clone(), nonce)
    .call()
    .await?;
  let eth_signed_message_hash = contract
    .getEthSignedMessageHash(message_hash)
    .call()
    .await?;
  let recovered = contract
    .recoverSigner(eth_signed_message_hash, signature.clone())
    .call()
    .await?;
  let verified = contract
    .verify(signer, to, amount, message, nonce, signature)
    .call()
    .await?;

  println!(
    "[VerifySignature] message_hash={message_hash}, recovered={recovered}, verified={verified}"
  );
  Ok(())
}
