use std::{fs, str::FromStr};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
  commitment_config::CommitmentConfig,
  instruction::{AccountMeta, Instruction},
  pubkey::Pubkey,
  signature::{Keypair, Signer},
  transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

// Constants
const KEYPAIR_FILE_PATH: &str = "~/solana-wallets/bob.json";

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct IntegerAccount {
  pub value: i64,
}

// Client-side instruction enum (for packing)
pub enum IntegerInstruction {
  Initialize { value: i64 },
  Add { value: i64 },
  Minus { value: i64 },
  Divide { value: i64 },
}

impl IntegerInstruction {
  pub fn pack(&self) -> Vec<u8> {
    let mut buf = vec![];
    match self {
      IntegerInstruction::Initialize { value } => {
        buf.push(0);
        buf.extend_from_slice(&value.to_le_bytes());
      }
      IntegerInstruction::Add { value } => {
        buf.push(1);
        buf.extend_from_slice(&value.to_le_bytes());
      }
      IntegerInstruction::Minus { value } => {
        buf.push(2);
        buf.extend_from_slice(&value.to_le_bytes());
      }
      IntegerInstruction::Divide { value } => {
        buf.push(3);
        buf.extend_from_slice(&value.to_le_bytes());
      }
    }
    buf
  }
}

fn load_keypair_from_file(file_path: &str) -> anyhow::Result<Keypair> {
  // Expand tilde to home directory
  let expanded_path = if file_path.starts_with('~') {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    file_path.replacen('~', &home, 1)
  } else {
    file_path.to_string()
  };

  // Read the keypair file
  let keypair_data = fs::read_to_string(&expanded_path)
    .map_err(|e| anyhow::anyhow!("Failed to read keypair file '{}': {}", expanded_path, e))?;

  // Parse the JSON array of bytes
  let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_data)
    .map_err(|e| anyhow::anyhow!("Failed to parse keypair file '{}': {}", expanded_path, e))?;

  // Create keypair from bytes
  Keypair::try_from(&keypair_bytes[..])
    .map_err(|e| anyhow::anyhow!("Failed to create keypair from bytes: {}", e))
}

fn main() -> anyhow::Result<()> {
  // 1. Set up client, payer, and program/account pubkeys
  let rpc_url = "http://localhost:8899";
  let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

  // Payer: load from keypair file or use Keypair::new() for demo (must be funded!)
  let payer = load_keypair_from_file(KEYPAIR_FILE_PATH)?;
  println!("Payer pubkey: {}", payer.pubkey());

  // The account storing your integer
  let integer_account = Keypair::new(); // Or load if already created
  println!("Integer account pubkey: {}", integer_account.pubkey());

  // Program id (deployed address)
    let program_id = Pubkey::from_str("CkNdo4Z3KEPKe5i9uRhiBDC6JAzL874jxSG31cwy1FYd")?;

  // 2. Create the data account (if needed)
  let (recent_blockhash, _) =
    client.get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())?;
  let space = 8; // Just i64, no additional header needed
  let rent = client.get_minimum_balance_for_rent_exemption(space)?;

  // Only needed if this is the first time!
  let create_account_ix = system_instruction::create_account(
    &payer.pubkey(),
    &integer_account.pubkey(),
    rent,
    space as u64,
    &program_id,
  );

  // 3. Compose your custom instruction
  let ix = Instruction::new_with_bytes(
    program_id,
    &IntegerInstruction::Initialize { value: 42 }.pack(), // <--- choose your action
    vec![AccountMeta::new(integer_account.pubkey(), false)],
  );

  // 4. First, create the account
  let create_tx = Transaction::new_signed_with_payer(
    &[create_account_ix],
    Some(&payer.pubkey()),
    &[&payer, &integer_account],
    recent_blockhash,
  );
  let create_sig = client.send_and_confirm_transaction(&create_tx)?;
  println!("Account creation tx sig: {}", create_sig);

  // 5. Then initialize it with our custom instruction
  let init_tx = Transaction::new_signed_with_payer(
    &[ix],
    Some(&payer.pubkey()),
    &[&payer],
    recent_blockhash,
  );
  let init_sig = client.send_and_confirm_transaction(&init_tx)?;
  println!("Initialize tx sig: {}", init_sig);

  // 6. Later: send add/minus/divide instructions
  let add_ix = Instruction::new_with_bytes(
    program_id,
    &IntegerInstruction::Add { value: 5 }.pack(),
    vec![AccountMeta::new(integer_account.pubkey(), false)],
  );
  let blockhash = client.get_latest_blockhash()?;
  let tx2 =
    Transaction::new_signed_with_payer(&[add_ix], Some(&payer.pubkey()), &[&payer], blockhash);
  let sig2 = client.send_and_confirm_transaction(&tx2)?;
  println!("Add tx sig: {}", sig2);

  // 7. Fetch and print the current value
  let acct = client.get_account(&integer_account.pubkey())?;
  let int_data = IntegerAccount::try_from_slice(&acct.data)?;
  println!("Current value: {}", int_data.value);

  Ok(())
}
