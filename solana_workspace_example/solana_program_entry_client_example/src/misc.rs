use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{msg, pubkey::Pubkey};

// fn sanitize_seed(input: &str, max_length: usize) -> String {
//   if input.len() <= max_length {
//     input.to_string()
//   } else {
//     // Take the first part and add a hash of the full string for uniqueness
//     let hash = {
//       use std::{
//         collections::hash_map::DefaultHasher,
//         hash::{Hash, Hasher},
//       };
//       let mut hasher = DefaultHasher::new();
//       input.hash(&mut hasher);
//       hasher.finish()
//     };

//     let prefix_len = max_length - 8; // Leave space for hash
//     let prefix = &input[.. prefix_len];
//     format!("{}{:08x}", prefix, hash)
//   }
// }

#[derive(BorshSerialize, BorshDeserialize, Debug, Default)]
pub struct CourseState {
  pub name: String,
  pub degree: String,
  pub institution: String,
  pub start_date: String,
}

// #[derive(BorshSerialize, BorshDeserialize, Debug)]
// pub enum CourseInstruction {
//   AddCourse {
//     name: String,
//     degree: String,
//     institution: String,
//     start_date: String,
//   },
//   UpdateCourse {
//     name: String,
//     degree: String,
//     institution: String,
//     start_date: String,
//   },
//   ReadCourse {
//     name: String,
//     start_date: String,
//   },
//   DeleteCourse {
//     name: String,
//     start_date: String,
//   },
// }

pub fn derive_pda_address(
  payload: &CourseState,
  program_id: &Pubkey,
) -> Result<(Pubkey, u8), Box<dyn std::error::Error>> {
  // Use the exact same logic as the program - no sanitization
  let (pda, bump_seed) = Pubkey::find_program_address(
    &[payload.name.as_bytes(), payload.start_date.as_bytes()],
    program_id,
  );

  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  Ok((pda, bump_seed))
}

pub fn derive_pda_from_name_and_date(
  name: &str,
  start_date: &str,
  program_id: &Pubkey,
) -> Result<(Pubkey, u8), Box<dyn std::error::Error>> {
  // Use the exact same logic as the program - no sanitization, just direct use
  let (pda, bump_seed) =
    Pubkey::find_program_address(&[name.as_bytes(), start_date.as_bytes()], program_id);

  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  Ok((pda, bump_seed))
}
