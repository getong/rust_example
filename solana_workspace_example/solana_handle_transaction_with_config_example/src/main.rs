// copy from https://github.com/joaquinsoza/soltrac
use std::str::FromStr;

use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_client::{rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
  // bs58,
  pubkey::Pubkey,
  signature::{
    // read_keypair_file,
    Signature,
  },
  // transaction::Transaction,
};
use solana_transaction_status::{
  EncodedTransaction,
  UiInstruction,
  UiMessage,
  UiParsedInstruction,
  UiParsedMessage,
  // UiPartiallyDecodedInstruction,
  UiTransactionEncoding,
  UiTransactionStatusMeta,
  // UiTransactionTokenBalance,
};
// use spl_token::state::{Account as TokenAccount, GenericTokenAccount};

#[derive(Parser)]
#[command(
  name = "soltrac",
  version = "0.1.0",
  author = "coderipper",
  about = "A CLI tool to copytrade a Solana wallet"
)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Start copytrading a wallet
  Copytrade {
    /// The wallet address to copytrade
    #[arg(value_name = "WALLET_ADDRESS")]
    wallet_address: String,
  },
}

#[tokio::main]
async fn main() -> Result<()> {
  let cli = Cli::parse();

  match &cli.command {
    Commands::Copytrade { wallet_address } => {
      let rpc_client = get_rpc_client();
      println!("Starting to copytrade wallet: {}", wallet_address);
      if let Err(e) = monitor_wallet(&rpc_client, wallet_address).await {
        eprintln!("Error: {:?}", e);
      }
    }
  }

  Ok(())
}

fn get_rpc_client() -> RpcClient {
  let rpc_url = "https://api.mainnet-beta.solana.com";
  RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed())
}

async fn monitor_wallet(rpc_client: &RpcClient, wallet_address: &str) -> Result<()> {
  let pubkey = wallet_address.parse::<Pubkey>()?;
  let mut processed_signatures = std::collections::HashSet::new();
  let mut first_poll = true;

  loop {
    println!("Polling wallet: {}", wallet_address);
    let signatures = rpc_client.get_signatures_for_address(&pubkey)?;

    if first_poll {
      // Initial fetch: Add all current signatures to processed set
      for signature_info in &signatures {
        processed_signatures.insert(signature_info.signature.clone());
      }
      first_poll = false;
      println!("First poll completed, tracking new transactions only.");
    } else {
      // Process new transactions only
      for signature_info in signatures {
        if !processed_signatures.contains(&signature_info.signature) {
          processed_signatures.insert(signature_info.signature.clone());
          println!(
            "New transaction signature detected: {}",
            signature_info.signature
          );

          // Here you can call process_transaction if it's a trade
          process_transaction(rpc_client, &signature_info.signature).await?;
        }
      }
    }

    // Sleep for a while before polling again
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
  }
}

async fn process_transaction(rpc_client: &RpcClient, signature: &str) -> Result<()> {
  println!("Processing transaction");

  let raydium_program_id = Pubkey::from_str("routeUGWgWzqBWFcrCfv8tritsqukccJPu3q5GPP3xS")?;
  let jupiter_program_id = Pubkey::from_str("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4")?;

  let transaction_with_meta = rpc_client.get_transaction_with_config(
    &Signature::from_str(signature)?,
    RpcTransactionConfig {
      encoding: Some(UiTransactionEncoding::JsonParsed),
      commitment: Some(CommitmentConfig::confirmed()),
      max_supported_transaction_version: Some(0),
    },
  )?;

  if let Some(meta) = &transaction_with_meta.transaction.meta {
    if let EncodedTransaction::Json(parsed_tx) = &transaction_with_meta.transaction.transaction {
      if let UiMessage::Parsed(message) = &parsed_tx.message {
        let mut is_raydium_tx = false;
        let mut is_jupiter_tx = false;

        // Check if any instruction interacts with Raydium or Jupiter
        for instruction in &message.instructions {
          if let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(partially_decoded)) =
            instruction
          {
            if partially_decoded.program_id == raydium_program_id.to_string() {
              is_raydium_tx = true;
            } else if partially_decoded.program_id == jupiter_program_id.to_string() {
              is_jupiter_tx = true;
            }
          }
        }

        // Route transaction to appropriate handler
        if is_raydium_tx {
          parse_raydium_swap(message, meta, rpc_client).await?;
        } else if is_jupiter_tx {
          parse_jupiter_swap(message)?;
        } else {
          println!("No Raydium or Jupiter instructions found.");
        }
      }
    }
  }

  Ok(())
}

// Method to parse swap details for Raydium transactions
async fn parse_raydium_swap(
  message: &UiParsedMessage,
  meta: &UiTransactionStatusMeta,
  _rpc_client: &RpcClient,
) -> Result<()> {
  println!("Parsing Raydium swap...");

  // Build a mapping from account indices to pubkeys
  let account_keys = &message.account_keys;
  let mut account_map = std::collections::HashMap::new();
  for (i, key) in account_keys.iter().enumerate() {
    account_map.insert(i, key.pubkey.clone());
  }

  // Identify the swap instruction
  for instruction in &message.instructions {
    if let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(partially_decoded)) =
      instruction
    {
      if partially_decoded.program_id == "routeUGWgWzqBWFcrCfv8tritsqukccJPu3q5GPP3xS" {
        println!("Instruction: {:?}", partially_decoded);
      }
    }
  }

  let (input_amount, output_amount) = calculate_token_balance_changes(meta)?;

  // Print the swap details
  println!("type: SWAP");
  // println!("input_token: \"{}\"", input_token_mint);
  println!("input_amount: \"{}\"", input_amount);
  // println!("output_token: \"{}\"", output_token_mint);
  println!("output_amount: \"{}\"", output_amount);

  Ok(())
}

// Helper function to get the mint address of a token account
// async fn get_token_account_mint(
//   rpc_client: &RpcClient,
//   token_account_pubkey_str: &str,
// ) -> Result<String> {
//   let token_account_pubkey = Pubkey::from_str(token_account_pubkey_str)?;
//   let account_data = rpc_client.get_account_data(&token_account_pubkey)?;

//   let token_account = TokenAccount::unpack_account_mint(&account_data).unwrap();
//   Ok(token_account.to_string())
// }

// Helper function to calculate the token balance changes for input and output tokens
fn calculate_token_balance_changes(meta: &UiTransactionStatusMeta) -> Result<(u64, u64)> {
  let input_amount: u64 = 0;
  let output_amount: u64 = 0;

  println!("meta: {:?}", meta);
  // Maybe do something with the meta data to calculate the input and output amounts and get the
  // token being used?

  Ok((input_amount, output_amount))
}

// Method to parse swap details for Jupiter transactions
fn parse_jupiter_swap(_message: &UiParsedMessage) -> Result<()> {
  println!("Parsing Jupiter swap...");

  // Implement similar logic for Jupiter swaps
  Ok(())
}

// async fn replicate_transaction(_rpc_client: &RpcClient, _tx: &Transaction) -> Result<()> {
//   // use solana_sdk::signer::Signer;

//   // Load your keypair
//   // let home_dir = dirs::home_dir().expect("Cannot find home directory");
//   // let keypair_path = home_dir.join(".config").join("solana").join("id.json");
//   // let _payer = read_keypair_file(keypair_path)?;

//   // // Construct a new transaction similar to the one detected
//   // let new_tx = Transaction::new_signed_with_payer(
//   //     &tx.message.instructions,
//   //     Some(&payer.pubkey()),
//   //     &[&payer],
//   //     tx.message.recent_blockhash,
//   // );

//   // Send the transaction
//   let signature = ""; // rpc_client.send_and_confirm_transaction(&new_tx)?;

//   println!("Replicated transaction with signature: {}", signature);

//   Ok(())
// }
