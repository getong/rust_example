use solana_program::{
  msg,
  program_error::ProgramError,
  pubkey::Pubkey,
  sysvar::{rent::Rent, Sysvar},
};

use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Debug, Default)]
pub struct CourseState {
  pub name: String,
  pub degree: String,
  pub institution: String,
  pub start_date: String,
}

pub enum CourseInstruction {
  AddCourse {
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  },
}

impl CourseInstruction {
  pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
    //
    let (&variant, rest) = input
      .split_first()
      .ok_or(ProgramError::InvalidInstructionData)?;
    let payload = CourseState::try_from_slice(rest).unwrap();
    //
    Ok(match variant {
      0 => Self::AddCourse {
        name: payload.name,
        degree: payload.degree,
        institution: payload.institution,
        start_date: payload.start_date,
      },
      _ => return Err(ProgramError::InvalidInstructionData),
    })
  }
}

pub fn derive_pda_address(payload: &CourseState, program_id: &Pubkey) -> (Pubkey, u8) {
  let (pda, bump_seed) = Pubkey::find_program_address(
    &[payload.name.as_bytes(), payload.start_date.as_bytes()],
    program_id,
  );
  //
  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  (pda, bump_seed)
}

pub fn calculate_acc_size_and_rent(payload: &CourseState) -> (usize, u64) {
  //
  let account_size: usize = (4 + payload.name.len())
    + (4 + payload.degree.len())
    + (4 + payload.institution.len())
    + (4 + payload.start_date.len());
  //
  let rent = Rent::get().unwrap();
  let rent_lamports = rent.minimum_balance(account_size);
  msg!(
    "Account size: {} and rent: {} lamports",
    account_size,
    rent_lamports
  );
  (account_size, rent_lamports)
}

pub fn my_try_from_slice_unchecked<T: borsh::BorshDeserialize>(
  data: &[u8],
) -> Result<T, ProgramError> {
  let mut data_mut = data;
  match T::deserialize(&mut data_mut) {
    Ok(result) => Ok(result),
    Err(_) => Err(ProgramError::InvalidInstructionData),
  }
}
