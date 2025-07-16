use std::str::FromStr;

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

use crate::misc::{CourseInstruction, CourseState, derive_pda_address};

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

fn main() {
  let payer = Keypair::new();
  // TODO
  let program_id = Pubkey::from_str("abdwwwww").unwrap();
  let solana_client = SolanaClient::new("https://api.devnet.solana.com", payer, program_id);

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
