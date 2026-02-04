use anyhow::Result;
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Signer, signer::keypair::Keypair};
use solana_system_interface::instruction as system_instruction;

#[tokio::main]
async fn main() -> Result<()> {
  // Generate sender and recipient keypairs
  let sender = Keypair::new();
  let recipient = Keypair::new();

  // Define the amount to transfer
  let transfer_amount = LAMPORTS_PER_SOL / 100; // 0.01 SOL

  // Create a transfer instruction for transferring SOL from sender to recipient
  let transfer_instruction =
    system_instruction::transfer(&sender.pubkey(), &recipient.pubkey(), transfer_amount);

  println!("{:#?}", transfer_instruction);

  Ok(())
}

// pub struct Instruction {
// Pubkey of the program that executes this instruction.
// pub program_id: Pubkey,
// Metadata describing accounts that should be passed to the program.
// pub accounts: Vec<AccountMeta>,
// Opaque data passed to the program for its own interpretation.
// pub data: Vec<u8>,
// }
//
// pub struct AccountMeta {
// An account's public key.
// pub pubkey: Pubkey,
// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
// pub is_signer: bool,
// True if the account data or metadata may be mutated during program execution.
// pub is_writable: bool,
// }

// copy from https://solana.com/zh/docs/core/transactions
