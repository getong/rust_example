use std::{fs, path::Path, str::FromStr};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::rpc_client::RpcClient;
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::{
  commitment_config::CommitmentConfig,
  instruction::AccountMeta,
  signature::{Keypair, Signer},
  transaction::Transaction,
};
use solana_sdk_ids::system_program;

mod misc;

use crate::misc::{derive_pda_address, derive_pda_from_name_and_date, CourseState};

// Constants for configuration
const SOLANA_RPC_URL: &str = "http://localhost:8899";
const WALLET_DIRECTORY: &str = "solana-wallets";
const WALLET_FILE_NAME: &str = "alice.json";
const SOLANA_PROGRAM_ID: &str = "3H298oTErSEpNwKgrbmcT7hzaSaRuApuebuc8BwJMTce";

// Constants for test course data
const COURSE_1_NAME: &str = "Rust Programming";
const COURSE_1_DEGREE: &str = "Bachelor";
const COURSE_1_INSTITUTION: &str = "University of Solana";
const COURSE_1_START_DATE: &str = "2025-01-01";

const COURSE_2_NAME: &str = "Advanced Solana Dev";
const COURSE_2_DEGREE: &str = "Master";
const COURSE_2_INSTITUTION: &str = "Best Blockchain University";
const COURSE_2_START_DATE: &str = "2025-02-01";

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

  pub fn course_exists(
    &self,
    course_state: &CourseState,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_address(course_state, &self.program_id)?;

    match self.client.get_account(&pda) {
      Ok(_account) => Ok(true),
      Err(_) => Ok(false),
    }
  }

  pub fn debug_account_data(
    &self,
    course_state: &CourseState,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_address(course_state, &self.program_id)?;

    // Get the raw account data
    let account_data = self.client.get_account_data(&pda)?;

    println!("Raw account data length: {}", account_data.len());
    println!(
      "Raw account data (first 50 bytes): {:?}",
      &account_data[.. account_data.len().min(50)]
    );

    Ok(())
  }

  pub fn get_course_data(
    &self,
    course_state: &CourseState,
  ) -> Result<CourseState, Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_address(course_state, &self.program_id)?;

    // Get the account data from the PDA
    let account_data = self.client.get_account_data(&pda)?;

    // Deserialize the data into CourseState
    let course_data = CourseState::try_from_slice(&account_data)?;

    Ok(course_data)
  }

  pub fn add_course(
    &self,
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let payload = CourseState {
      name: name.clone(),
      degree: degree.clone(),
      institution: institution.clone(),
      start_date: start_date.clone(),
    };

    let (pda, _bump) = derive_pda_address(&payload, &self.program_id)?;

    // Create instruction data manually for AddCourse
    let mut instruction_data = Vec::new();
    instruction_data.push(0u8); // AddCourse variant is 0

    // Serialize the CourseState payload
    payload.serialize(&mut instruction_data)?;

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

    let result = self.client.send_and_confirm_transaction(&transaction)?;
    println!("Transaction confirmed with signature: {}", result);
    Ok(())
  }

  pub fn update_course(
    &self,
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, &self.program_id)?;

    // Create instruction data manually for UpdateCourse
    let mut instruction_data = Vec::new();
    instruction_data.push(1u8); // UpdateCourse variant is 1

    // Serialize the CourseState payload
    let payload = CourseState {
      name,
      degree,
      institution,
      start_date,
    };
    payload.serialize(&mut instruction_data)?;

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

    let result = self.client.send_and_confirm_transaction(&transaction)?;
    println!("Update transaction confirmed with signature: {}", result);
    Ok(())
  }
  pub fn read_course(
    &self,
    name: String,
    start_date: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, &self.program_id)?;

    // Create instruction data manually for ReadCourse
    let mut instruction_data = Vec::new();
    instruction_data.push(2u8); // ReadCourse variant is 2

    // Serialize name length and name
    let name_bytes = name.as_bytes();
    instruction_data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    instruction_data.extend_from_slice(name_bytes);

    // Serialize start_date length and start_date
    let start_date_bytes = start_date.as_bytes();
    instruction_data.extend_from_slice(&(start_date_bytes.len() as u32).to_le_bytes());
    instruction_data.extend_from_slice(start_date_bytes);

    let pda_meta = AccountMeta::new_readonly(pda, false);
    let accounts = vec![pda_meta];

    let instruction = Instruction {
      program_id: self.program_id.clone(),
      accounts,
      data: instruction_data,
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));

    let recent_blockhash = self.client.get_latest_blockhash()?;
    transaction.sign(&[&self.payer], recent_blockhash);

    let result = self.client.send_and_confirm_transaction(&transaction)?;
    println!("Read transaction confirmed with signature: {}", result);
    Ok(())
  }

  pub fn delete_course(
    &self,
    name: String,
    start_date: String,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, &self.program_id)?;

    // Create instruction data manually for DeleteCourse
    let mut instruction_data = Vec::new();
    instruction_data.push(3u8); // DeleteCourse variant is 3

    // Serialize name length and name
    let name_bytes = name.as_bytes();
    instruction_data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    instruction_data.extend_from_slice(name_bytes);

    // Serialize start_date length and start_date
    let start_date_bytes = start_date.as_bytes();
    instruction_data.extend_from_slice(&(start_date_bytes.len() as u32).to_le_bytes());
    instruction_data.extend_from_slice(start_date_bytes);

    let payer_meta = AccountMeta::new(self.payer.pubkey(), true);
    let pda_meta = AccountMeta::new(pda, false);

    let accounts = vec![payer_meta, pda_meta];

    let instruction = Instruction {
      program_id: self.program_id.clone(),
      accounts,
      data: instruction_data,
    };

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&self.payer.pubkey()));

    let recent_blockhash = self.client.get_latest_blockhash()?;
    transaction.sign(&[&self.payer], recent_blockhash);

    let result = self.client.send_and_confirm_transaction(&transaction)?;
    println!("Delete transaction confirmed with signature: {}", result);
    Ok(())
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
  let keypair_path = format!("{}/{}/{}", home_dir, WALLET_DIRECTORY, WALLET_FILE_NAME);

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

  let program_id = Pubkey::from_str(SOLANA_PROGRAM_ID).unwrap();
  let solana_client = SolanaClient::new(SOLANA_RPC_URL, payer, program_id);

  // Create CourseState for checking and retrieving data
  let course_state = CourseState {
    name: COURSE_1_NAME.to_string(),
    degree: COURSE_1_DEGREE.to_string(),
    institution: COURSE_1_INSTITUTION.to_string(),
    start_date: COURSE_1_START_DATE.to_string(),
  };

  // Check if course already exists
  match solana_client.course_exists(&course_state) {
    Ok(exists) => {
      if exists {
        println!("Course already exists! Retrieving existing data...");

        // Retrieve and display the stored course data
        match solana_client.get_course_data(&course_state) {
          Ok(stored_course) => {
            println!("\n=== Retrieved Course Data ===");
            println!("Name: {}", stored_course.name);
            println!("Degree: {}", stored_course.degree);
            println!("Institution: {}", stored_course.institution);
            println!("Start Date: {}", stored_course.start_date);
            println!("============================");
          }
          Err(e) => {
            eprintln!("Error retrieving course data: {:?}", e);
            // Debug the raw account data to understand the issue
            println!("Debugging raw account data for existing course...");
            if let Err(debug_err) = solana_client.debug_account_data(&course_state) {
              eprintln!("Error debugging account data: {:?}", debug_err);
            }
          }
        }
      } else {
        println!("Course doesn't exist. Adding new course...");

        let result = solana_client.add_course(
          COURSE_1_NAME.to_string(),
          COURSE_1_DEGREE.to_string(),
          COURSE_1_INSTITUTION.to_string(),
          COURSE_1_START_DATE.to_string(),
        );

        match result {
          Ok(_) => {
            println!("Course added successfully!");

            // Read data from chain immediately after adding
            println!("Reading data from chain after ADD operation...");
            match solana_client.get_course_data(&course_state) {
              Ok(stored_course) => {
                println!("\n=== Retrieved Course Data ===");
                println!("Name: {}", stored_course.name);
                println!("Degree: {}", stored_course.degree);
                println!("Institution: {}", stored_course.institution);
                println!("Start Date: {}", stored_course.start_date);
                println!("============================");
              }
              Err(e) => eprintln!("Error retrieving course data: {:?}", e),
            }
          }
          Err(e) => eprintln!("Error adding course: {:?}", e),
        }
      }
    }
    Err(e) => eprintln!("Error checking if course exists: {:?}", e),
  }

  // Try adding a different course to demonstrate the system works
  println!("\n--- Trying to add a different course ---");
  let course_state2 = CourseState {
    name: COURSE_2_NAME.to_string(),
    degree: COURSE_2_DEGREE.to_string(),
    institution: COURSE_2_INSTITUTION.to_string(),
    start_date: COURSE_2_START_DATE.to_string(),
  };

  match solana_client.course_exists(&course_state2) {
    Ok(exists) => {
      if exists {
        println!("Second course already exists! Retrieving existing data...");

        match solana_client.get_course_data(&course_state2) {
          Ok(stored_course) => {
            println!("\n=== Retrieved Second Course Data ===");
            println!("Name: {}", stored_course.name);
            println!("Degree: {}", stored_course.degree);
            println!("Institution: {}", stored_course.institution);
            println!("Start Date: {}", stored_course.start_date);
            println!("=====================================");
          }
          Err(e) => eprintln!("Error retrieving second course data: {:?}", e),
        }
      } else {
        println!("Second course doesn't exist. Adding new course...");

        let result2 = solana_client.add_course(
          COURSE_2_NAME.to_string(),
          COURSE_2_DEGREE.to_string(),
          COURSE_2_INSTITUTION.to_string(),
          COURSE_2_START_DATE.to_string(),
        );

        match result2 {
          Ok(_) => {
            println!("Second course added successfully!");

            // Read data from chain immediately after adding second course
            println!("Reading second course data from chain after ADD operation...");
            match solana_client.get_course_data(&course_state2) {
              Ok(stored_course) => {
                println!("\n=== Retrieved Second Course Data ===");
                println!("Name: {}", stored_course.name);
                println!("Degree: {}", stored_course.degree);
                println!("Institution: {}", stored_course.institution);
                println!("Start Date: {}", stored_course.start_date);
                println!("=====================================");
              }
              Err(e) => eprintln!("Error retrieving second course data: {:?}", e),
            }
          }
          Err(e) => eprintln!("Error adding second course: {:?}", e),
        }
      }
    }
    Err(e) => eprintln!("Error checking if second course exists: {:?}", e),
  }

  // Demonstrate CRUD operations
  println!("\n--- Demonstrating CRUD Operations ---");

  // Update the first course
  println!("\n=== UPDATE Operation ===");
  match solana_client.update_course(
    COURSE_1_NAME.to_string(),
    "Master".to_string(),      // 6 chars vs original 8 chars ("Bachelor")
    "Solana Univ".to_string(), // 12 chars vs original 20 chars
    COURSE_1_START_DATE.to_string(),
  ) {
    Ok(_) => {
      println!("Course updated successfully!");

      // Read data from chain immediately after UPDATE operation
      println!("Reading data from chain after UPDATE operation...");
      // Create a CourseState with the updated values for proper PDA derivation
      let updated_course_state = CourseState {
        name: COURSE_1_NAME.to_string(),
        degree: "Master".to_string(),
        institution: "Solana Univ".to_string(),
        start_date: COURSE_1_START_DATE.to_string(),
      };

      match solana_client.get_course_data(&updated_course_state) {
        Ok(updated_course) => {
          println!("\n=== Updated Course Data ===");
          println!("Name: {}", updated_course.name);
          println!("Degree: {}", updated_course.degree);
          println!("Institution: {}", updated_course.institution);
          println!("Start Date: {}", updated_course.start_date);
          println!("===============================");
        }
        Err(e) => {
          eprintln!("Error retrieving updated course data: {:?}", e);
          // Debug the raw account data
          println!("Debugging raw account data after UPDATE...");
          if let Err(debug_err) = solana_client.debug_account_data(&course_state) {
            eprintln!("Error debugging account data: {:?}", debug_err);
          }

          // Try with original course state
          println!("Trying with original course state for PDA derivation...");
          match solana_client.get_course_data(&course_state) {
            Ok(course) => {
              println!("\n=== Course Data (via original PDA) ===");
              println!("Name: {}", course.name);
              println!("Degree: {}", course.degree);
              println!("Institution: {}", course.institution);
              println!("Start Date: {}", course.start_date);
              println!("======================================");
            }
            Err(e2) => eprintln!("Error with original PDA too: {:?}", e2),
          }
        }
      }
    }
    Err(e) => eprintln!("Error updating course: {:?}", e),
  }

  // Read operation using the program's read instruction
  println!("\n=== READ Operation (using program instruction) ===");
  match solana_client.read_course(COURSE_1_NAME.to_string(), COURSE_1_START_DATE.to_string()) {
    Ok(_) => println!("Read operation completed successfully! Check program logs for details."),
    Err(e) => eprintln!("Error reading course: {:?}", e),
  }

  // Delete the second course
  println!("\n=== DELETE Operation ===");
  match solana_client.delete_course(COURSE_2_NAME.to_string(), COURSE_2_START_DATE.to_string()) {
    Ok(_) => {
      println!("Second course deleted successfully!");

      // Try to read the deleted course to confirm deletion
      match solana_client.course_exists(&course_state2) {
        Ok(exists) => {
          if exists {
            println!("WARNING: Course still exists after deletion attempt!");
          } else {
            println!("Confirmed: Course has been successfully deleted.");
          }
        }
        Err(e) => eprintln!("Error checking if deleted course exists: {:?}", e),
      }
    }
    Err(e) => eprintln!("Error deleting course: {:?}", e),
  }

  println!("\n--- CRUD Operations Demo Complete ---");
}
