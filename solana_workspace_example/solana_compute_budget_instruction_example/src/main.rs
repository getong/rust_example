use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
  commitment_config::CommitmentConfig,
  compute_budget::ComputeBudgetInstruction,
  native_token::LAMPORTS_PER_SOL,
  signature::{Keypair, Signer},
  transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let client = RpcClient::new_with_commitment(
    String::from("http://localhost:8899"),
    CommitmentConfig::confirmed(),
  );

  let sender = Keypair::new();
  let recipient = Keypair::new();

  let airdrop_signature = client
    .request_airdrop(&sender.pubkey(), LAMPORTS_PER_SOL)
    .await?;

  loop {
    let confirmed = client.confirm_transaction(&airdrop_signature).await?;
    if confirmed {
      break;
    }
  }

  // Create compute budget instructions
  let limit_instruction = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
  let price_instruction = ComputeBudgetInstruction::set_compute_unit_price(1);

  let transfer_amount = LAMPORTS_PER_SOL / 100;
  let transfer_instruction =
    system_instruction::transfer(&sender.pubkey(), &recipient.pubkey(), transfer_amount);

  let recent_blockhash = client.get_latest_blockhash().await?;

  let mut transaction = Transaction::new_with_payer(
    &[limit_instruction, price_instruction, transfer_instruction],
    Some(&sender.pubkey()),
  );
  transaction.sign(&[&sender], recent_blockhash);

  let signature = client.send_and_confirm_transaction(&transaction).await?;
  println!("Transaction Signature: {}", signature);

  Ok(())
}

// copy from https://solana.com/zh/docs/core/fees
