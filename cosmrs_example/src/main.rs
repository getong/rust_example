use std::fmt::Write;

use cosmrs::{
  AccountId, Coin,
  bank::MsgSend,
  crypto::secp256k1,
  tx::{self, Fee, Msg, SignDoc, SignerInfo, Tx},
};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let sender_private_key = secp256k1::SigningKey::from_slice(&[7u8; 32])?;
  let sender_public_key = sender_private_key.public_key();
  let sender_account_id = sender_public_key.account_id("cosmos")?;

  let recipient_account_id =
    "cosmos19dyl0uyzes4k23lscla02n06fc22h4uqsdwq6z".parse::<AccountId>()?;

  let transfer_amount = Coin {
    amount: 1_500_000u128,
    denom: "uatom".parse()?,
  };
  let fee_amount = Coin {
    amount: 750u128,
    denom: "uatom".parse()?,
  };

  let msg_send = MsgSend {
    from_address: sender_account_id.clone(),
    to_address: recipient_account_id.clone(),
    amount: vec![transfer_amount.clone()],
  };

  let chain_id = "cosmoshub-4".parse()?;
  let account_number = 7;
  let sequence_number = 3;
  let gas_limit = 120_000u64;
  let timeout_height = 123_456u32;
  let memo = "CosmRS bank send example";

  let tx_body = tx::Body::new(vec![msg_send.to_any()?], memo, timeout_height);
  let signer_info = SignerInfo::single_direct(Some(sender_public_key), sequence_number);
  let auth_info = signer_info.auth_info(Fee::from_amount_and_gas(fee_amount.clone(), gas_limit));

  let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number)?;
  let tx_raw = sign_doc.sign(&sender_private_key)?;
  let tx_bytes = tx_raw.to_bytes()?;
  let parsed_tx = Tx::from_bytes(&tx_bytes)?;

  println!("=== CosmRS Cosmos Bank Tx Example ===");
  println!("chain_id        : {chain_id}");
  println!("sender          : {sender_account_id}");
  println!("recipient       : {recipient_account_id}");
  println!(
    "transfer        : {} {}",
    transfer_amount.amount, transfer_amount.denom
  );
  println!(
    "fee             : {} {}",
    fee_amount.amount, fee_amount.denom
  );
  println!("account_number  : {account_number}");
  println!("sequence_number : {sequence_number}");
  println!("gas_limit       : {gas_limit}");
  println!("timeout_height  : {timeout_height}");
  println!("memo            : {memo}");
  println!("messages        : {}", parsed_tx.body.messages.len());
  println!("signatures      : {}", parsed_tx.signatures.len());
  println!("tx_bytes_len    : {}", tx_bytes.len());
  println!("tx_bytes_hex    : {}", hex_encode(&tx_bytes));

  assert_eq!(parsed_tx.body, tx_body);
  assert_eq!(parsed_tx.auth_info, auth_info);

  println!();
  println!("note:");
  println!("1. it uses `MsgSend`。");
  println!("2. `account_number` , `sequence_number` check on chain。");
  println!("3. the generated `tx_bytes` can use as transaction info。");

  Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
  let mut output = String::with_capacity(bytes.len() * 2);

  for byte in bytes {
    write!(&mut output, "{byte:02x}").expect("writing to a String should not fail");
  }

  output
}
