use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
  native_token::LAMPORTS_PER_SOL, signature::Signer, signer::keypair::Keypair,
  transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

#[tokio::main]
async fn main() -> Result<()> {
  let connection = RpcClient::new_with_commitment(
    "http://localhost:8899".to_string(),
    CommitmentConfig::confirmed(),
  );

  // Fetch the latest blockhash and last valid block height
  let blockhash = connection.get_latest_blockhash().await?;

  // Generate sender and recipient keypairs
  let sender = Keypair::new();
  let recipient = Keypair::new();

  // Create a transfer instruction for transferring SOL from sender to recipient
  let transfer_instruction = system_instruction::transfer(
    &sender.pubkey(),
    &recipient.pubkey(),
    LAMPORTS_PER_SOL / 100, // 0.01 SOL
  );

  let mut transaction =
    Transaction::new_with_payer(&[transfer_instruction], Some(&sender.pubkey()));
  transaction.sign(&[&sender], blockhash);

  println!("{:#?}", transaction);

  Ok(())
}

/*
pub struct Transaction {
  #[wasm_bindgen(skip)]
  #[serde(with = "short_vec")]
  pub signatures: Vec<Signature>,

  #[wasm_bindgen(skip)]
  pub message: Message,
}

pub struct Message {
  /// The message header, identifying signed and read-only `account_keys`.
  pub header: MessageHeader,

  /// All the account keys used by this transaction.
  #[serde(with = "short_vec")]
  pub account_keys: Vec<Pubkey>,

  /// The id of a recent ledger entry.
  pub recent_blockhash: Hash,

  /// Programs that will be executed in sequence and committed in
  /// one atomic transaction if all succeed.
  #[serde(with = "short_vec")]
  pub instructions: Vec<CompiledInstruction>,
}
pub struct MessageHeader {
  /// The number of signatures required for this message to be considered
  /// valid. The signers of those signatures must match the first
  /// `num_required_signatures` of [`Message::account_keys`].
  pub num_required_signatures: u8,

  /// The last `num_readonly_signed_accounts` of the signed keys are read-only
  /// accounts.
  pub num_readonly_signed_accounts: u8,

  /// The last `num_readonly_unsigned_accounts` of the unsigned keys are
  /// read-only accounts.
  pub num_readonly_unsigned_accounts: u8,
}

pub struct CompiledInstruction {
  /// Index into the transaction keys array indicating the program account that executes this
  /// instruction.
  pub program_id_index: u8,
  /// Ordered indices into the transaction keys array indicating which accounts to pass to the
  /// program.
  #[serde(with = "short_vec")]
  pub accounts: Vec<u8>,
  /// The program input data.
  #[serde(with = "short_vec")]
  pub data: Vec<u8>,
}
*/
// copy from https://solana.com/zh/docs/core/transactions
