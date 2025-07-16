use std::{fmt, fmt::Display, rc::Rc};

use anchor_client::{
  Client, Cluster,
  solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature,
    signature::{Signature, Signer, SignerError},
  },
};
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;

/// A tokio compatible wrapper for `anchor_client::solana_sdk::signature::Keypair`
///
/// The standard Keypair is not Sendable and cannot be used from
/// within a Tokio runtime.
///
/// This Keypair works by storing the keypair bytes and only
/// rebuilding the original anchor_client Keypair when inside
/// the synchronous context of the Signer trait
#[derive(Clone)]
pub struct Keypair {
  keypair: [u8; 64],
}

// trait SendableSigner: Send + Sync + Signer {}
// impl SendableSigner for Keypair {}

impl Display for Keypair {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let keypair = signature::Keypair::try_from(&self.keypair[..]).map_err(|_| fmt::Error)?;
    write!(f, "{}", keypair.to_base58_string())
  }
}

impl Keypair {
  pub fn new<T: AsRef<signature::Keypair>>(keypair: T) -> Self {
    Self {
      keypair: keypair.as_ref().to_bytes(),
    }
  }

  pub fn from_keypair(keypair: &signature::Keypair) -> Self {
    Self {
      keypair: keypair.to_bytes(),
    }
  }

  pub fn from_base58_string(s: &str) -> Self {
    Self {
      keypair: signature::Keypair::from_base58_string(s).to_bytes(),
    }
  }
}

impl Signer for Keypair {
  fn try_pubkey(&self) -> std::result::Result<Pubkey, SignerError> {
    // Convert the stored keypair bytes back to a Keypair
    let keypair = signature::Keypair::try_from(&self.keypair[..])
      .map_err(|e| SignerError::Custom(e.to_string()))?;

    // Return the public key of the keypair
    Ok(keypair.pubkey())
  }

  fn try_sign_message(&self, message: &[u8]) -> std::result::Result<Signature, SignerError> {
    // Convert the stored keypair bytes back to a Keypair
    let keypair = signature::Keypair::try_from(&self.keypair[..])
      .map_err(|e| SignerError::Custom(e.to_string()))?;

    // Sign the message with the keypair
    Ok(keypair.try_sign_message(message)?)
  }

  fn is_interactive(&self) -> bool {
    // This method should return true if the signer requires user interaction to sign messages.
    // In this case, we assume that it does not require user interaction, so we return false.
    false
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  // Example program ID - replace with your actual program ID
  let program_id = Pubkey::new_unique();

  // Create a payer keypair
  let payer = Rc::new(Keypair::from_keypair(&signature::Keypair::new()));

  // Create the anchor client
  let client = Client::new_with_options(Cluster::Localnet, payer, CommitmentConfig::processed());
  let _program = client.program(program_id)?;

  // Create some dummy keypairs for the example
  let dummy_a = Keypair::from_keypair(&signature::Keypair::new());
  let dummy_b = Keypair::from_keypair(&signature::Keypair::new());

  println!("Program ID: {}", program_id);
  println!("Dummy A pubkey: {}", dummy_a.pubkey());
  println!("Dummy B pubkey: {}", dummy_b.pubkey());

  // Example of getting the latest blockhash
  let rpc_client = RpcClient::new_with_commitment(
    "http://localhost:8899".to_string(),
    CommitmentConfig::processed(),
  );

  let (blockhash, _) = rpc_client
    .get_latest_blockhash_with_commitment(CommitmentConfig::processed())
    .await?;

  println!("Latest blockhash: {}", blockhash);

  // Example of creating a simple transaction (without actual instruction)
  // This is just a demonstration of how to use the async client

  // Get account info for demonstration
  let account_info = rpc_client.get_account(&dummy_a.pubkey()).await;
  match account_info {
    Ok(account) => {
      println!("Account found with lamports: {}", account.lamports);
    }
    Err(e) => {
      println!("Error getting account: {}", e);
    }
  }

  println!("Async anchor client example completed successfully!");
  Ok(())
}
