use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{msg, pubkey::Pubkey};

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

pub fn derive_pda_address(payload: &CourseState, program_id: &Pubkey) -> (Pubkey, u8) {
  let (pda, bump_seed) = Pubkey::find_program_address(
    &[payload.name.as_bytes(), payload.start_date.as_bytes()],
    program_id,
  );

  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  (pda, bump_seed)
}
