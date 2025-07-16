use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{msg, pubkey::Pubkey};

fn sanitize_seed(input: &str, max_length: usize) -> String {
  if input.len() <= max_length {
    input.to_string()
  } else {
    // Take the first part and add a hash of the full string for uniqueness
    let hash = {
      use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
      };
      let mut hasher = DefaultHasher::new();
      input.hash(&mut hasher);
      hasher.finish()
    };

    let prefix_len = max_length - 8; // Leave space for hash
    let prefix = &input[.. prefix_len];
    format!("{}{:08x}", prefix, hash)
  }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Default)]
pub struct CourseState {
  pub name: String,
  pub degree: String,
  pub institution: String,
  pub start_date: String,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum CourseInstruction {
  AddCourse {
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  },
}

pub fn derive_pda_address(
  payload: &CourseState,
  program_id: &Pubkey,
) -> Result<(Pubkey, u8), Box<dyn std::error::Error>> {
  // Ensure course name is not too long for PDA generation (max 32 bytes per seed)
  let truncated_name = if payload.name.len() > 32 {
    &payload.name[.. 32]
  } else {
    &payload.name
  };

  let truncated_start_date = if payload.start_date.len() > 32 {
    &payload.start_date[.. 32]
  } else {
    &payload.start_date
  };

  // Sanitize seeds to ensure they don't exceed Solana's length limits
  let sanitized_name = sanitize_seed(truncated_name, 32);
  let sanitized_start_date = sanitize_seed(truncated_start_date, 32);

  // Use the original seed combination for compatibility with the program
  let original_seeds = &[sanitized_name.as_bytes(), sanitized_start_date.as_bytes()];

  match Pubkey::try_find_program_address(original_seeds, program_id) {
    Some((pda, bump_seed)) => {
      msg!("pda is {} and bump seed is {}", pda, bump_seed);
      Ok((pda, bump_seed))
    }
    None => {
      // Fallback: try with just the truncated name if the original fails
      let fallback_seeds = &[b"course-fallback", sanitized_name.as_bytes()];

      match Pubkey::try_find_program_address(fallback_seeds, program_id) {
        Some((pda, bump_seed)) => {
          msg!("fallback pda is {} and bump seed is {}", pda, bump_seed);
          Ok((pda, bump_seed))
        }
        None => {
          // Last resort: use a very short identifier
          let short_name = if sanitized_name.len() > 16 {
            &sanitized_name[.. 16]
          } else {
            &sanitized_name
          };

          let final_seeds = &[b"course-final", short_name.as_bytes()];

          match Pubkey::try_find_program_address(final_seeds, program_id) {
            Some((pda, bump_seed)) => {
              msg!("final pda is {} and bump seed is {}", pda, bump_seed);
              Ok((pda, bump_seed))
            }
            None => Err("Unable to derive PDA address".into()),
          }
        }
      }
    }
  }
}
