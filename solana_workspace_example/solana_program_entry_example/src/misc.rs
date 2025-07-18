use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
  msg,
  program_error::ProgramError,
  pubkey::Pubkey,
  sysvar::{rent::Rent, Sysvar},
};

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
  UpdateCourse {
    name: String,
    degree: String,
    institution: String,
    start_date: String,
  },
  ReadCourse {
    name: String,
    start_date: String,
  },
  DeleteCourse {
    name: String,
    start_date: String,
  },
}

impl CourseInstruction {
  pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
    let (&variant, rest) = input
      .split_first()
      .ok_or(ProgramError::InvalidInstructionData)?;

    Ok(match variant {
      0 => {
        let payload = CourseState::try_from_slice(rest)?;
        Self::AddCourse {
          name: payload.name,
          degree: payload.degree,
          institution: payload.institution,
          start_date: payload.start_date,
        }
      }
      1 => {
        let payload = CourseState::try_from_slice(rest)?;
        Self::UpdateCourse {
          name: payload.name,
          degree: payload.degree,
          institution: payload.institution,
          start_date: payload.start_date,
        }
      }
      2 => {
        // For read and delete, we only need name and start_date
        let name_len = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let name = String::from_utf8(rest[4 .. 4 + name_len].to_vec())
          .map_err(|_| ProgramError::InvalidInstructionData)?;

        let start_date_len = u32::from_le_bytes([
          rest[4 + name_len],
          rest[5 + name_len],
          rest[6 + name_len],
          rest[7 + name_len],
        ]) as usize;
        let start_date =
          String::from_utf8(rest[8 + name_len .. 8 + name_len + start_date_len].to_vec())
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        Self::ReadCourse { name, start_date }
      }
      3 => {
        // For delete, we only need name and start_date
        let name_len = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let name = String::from_utf8(rest[4 .. 4 + name_len].to_vec())
          .map_err(|_| ProgramError::InvalidInstructionData)?;

        let start_date_len = u32::from_le_bytes([
          rest[4 + name_len],
          rest[5 + name_len],
          rest[6 + name_len],
          rest[7 + name_len],
        ]) as usize;
        let start_date =
          String::from_utf8(rest[8 + name_len .. 8 + name_len + start_date_len].to_vec())
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        Self::DeleteCourse { name, start_date }
      }
      _ => return Err(ProgramError::InvalidInstructionData),
    })
  }
}

pub fn derive_pda_address(payload: &CourseState, program_id: &Pubkey) -> (Pubkey, u8) {
  let (pda, bump_seed) = Pubkey::find_program_address(
    &[payload.name.as_bytes(), payload.start_date.as_bytes()],
    program_id,
  );

  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  (pda, bump_seed)
}

pub fn calculate_acc_size_and_rent(payload: &CourseState) -> (usize, u64) {
  let account_size: usize = (4 + payload.name.len())
    + (4 + payload.degree.len())
    + (4 + payload.institution.len())
    + (4 + payload.start_date.len());

  match Rent::get() {
    Ok(rent) => {
      let rent_lamports = rent.minimum_balance(account_size);
      msg!(
        "Account size: {} and rent: {} lamports",
        account_size,
        rent_lamports
      );
      (account_size, rent_lamports)
    }
    Err(_) => (0, 0),
  }
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

pub fn derive_pda_from_name_and_date(
  name: &str,
  start_date: &str,
  program_id: &Pubkey,
) -> (Pubkey, u8) {
  let (pda, bump_seed) =
    Pubkey::find_program_address(&[name.as_bytes(), start_date.as_bytes()], program_id);

  msg!("pda is {} and bump seed is {}", pda, bump_seed);
  (pda, bump_seed)
}
