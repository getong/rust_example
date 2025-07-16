use std::{fs, path::Path, str::FromStr};

use borsh::BorshSerialize;
use solana_client::{client_error::ClientError, rpc_client::RpcClient};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::{
  commitment_config::CommitmentConfig,
  instruction::AccountMeta,
  signature::{Keypair, Signer},
  transaction::Transaction,
};
use solana_sdk_ids::system_program;

mod misc;

use crate::misc::{derive_pda_address, CourseInstruction, CourseState};

pub struct SolanaClient {
  client: RpcClient,
  payer: Keypair,
  program_id: Pubkey,
}

impl SolanaClient {
  pub fn new(rpc_url: &str, payer: Keypair, program_id: Pubkey) -> Self {
    let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    SolanaClient {
      client,
      payer,
      program_id,
    }
  }

  pub fn add_course(
    &self,
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  ) -> Result<(), ClientError> {
    let payload = CourseState {
      name: name.clone(),
      degree: degree.clone(),
      institution: institution.clone(),
      start_date: start_date.clone(),
    };

    let (pda, _bump) = derive_pda_address(&payload, &self.program_id);

    let course_instruction = CourseInstruction::AddCourse {
      name,
      degree,
      institution,
      start_date,
    };

    let mut instruction_data = Vec::new();
    course_instruction.serialize(&mut instruction_data).unwrap();
    let payer_meta = AccountMeta::new(self.payer.pubkey(), true);
    let pda_meta = AccountMeta::new(pda, false);
    let system_program_meta = AccountMeta::new_readonly(system_program::id(), false);

    let accounts = vec![payer_meta, pda_meta, system_program_meta];

    let instruction = Instruction {
      program_id: self.program_id.clone(),
      accounts,
      data: instruction_data,
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));

    let recent_blockhash = self.client.get_latest_blockhash()?;
    transaction.sign(&[&self.payer], recent_blockhash);

    let result = self.client.send_and_confirm_transaction(&transaction);

    match result {
      Ok(signature) => {
        println!("Transaction confirmed with signature: {}", signature);
        Ok(())
      }
      Err(e) => Err(e),
    }
  }
}

fn load_keypair_from_file<P: AsRef<Path>>(path: P) -> Result<Keypair, Box<dyn std::error::Error>> {
  let file_content = fs::read_to_string(path)?;
  let bytes: Vec<u8> = serde_json::from_str(&file_content)?;
  Ok(Keypair::try_from(&bytes[..])?)
}

fn main() {
  // Load environment variables from .env file if it exists
  dotenvy::dotenv().ok();

  // Load keypair from file system
  let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
  let keypair_path = format!("{}/solana-wallets/alice.json", home_dir);

  let payer = match load_keypair_from_file(&keypair_path) {
    Ok(keypair) => {
      println!("Successfully loaded keypair from: {}", keypair_path);
      keypair
    }
    Err(e) => {
      eprintln!("Failed to load keypair from {}: {}", keypair_path, e);
      eprintln!("Falling back to generating a new keypair...");
      Keypair::new()
    }
  };

  let program_id = Pubkey::from_str("8JgSyo7yEeGNrThPWTkDB1AxwVYKKXDGjxaxDMSz2mzr").unwrap();
  let solana_client = SolanaClient::new("http://localhost:8899", payer, program_id);

  let result = solana_client.add_course(
    "Rust Programming".to_string(),
    "Bachelor".to_string(),
    "University of Solana".to_string(),
    "2025-01-01".to_string(),
  );

  match result {
    Ok(_) => println!("Course added successfully!"),
    Err(e) => eprintln!("Error adding course: {:?}", e),
  }
}
