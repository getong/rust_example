use borsh::BorshSerialize;
use solana_program::{
  account_info::{next_account_info, AccountInfo},
  entrypoint,
  entrypoint::ProgramResult,
  msg,
  program::invoke_signed,
  program_error::ProgramError,
  pubkey::Pubkey,
  system_instruction,
};

pub mod misc;

use misc::{
  calculate_acc_size_and_rent, derive_pda_address, my_try_from_slice_unchecked, CourseInstruction,
  CourseState,
};

entrypoint!(process_instruction);

fn process_instruction(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  instruction_data: &[u8],
) -> ProgramResult {
  msg!("start execution...");

  let account_info_iter = &mut accounts.iter();
  let initializer = next_account_info(account_info_iter)?;
  let pda_account = next_account_info(account_info_iter)?;
  let system_program = next_account_info(account_info_iter)?;

  let payload;
  let instruction = CourseInstruction::unpack(instruction_data)?;
  match instruction {
    CourseInstruction::AddCourse {
      name,
      degree,
      institution,
      start_date,
    } => {
      payload = CourseState {
        name,
        degree,
        institution,
        start_date,
      };
      msg!("This is an instruction to add a course...");
    }
  }
  //
  let (pda, bump) = derive_pda_address(&payload, program_id);
  let (size, rent) = calculate_acc_size_and_rent(&payload);
  //
  if *pda_account.key != pda {
    return Err(ProgramError::InvalidArgument);
  }
  msg!("pda account and key identical");
  //
  invoke_signed(
    &system_instruction::create_account(
      initializer.key,
      pda_account.key,
      rent,
      size.try_into().unwrap(),
      program_id,
    ),
    &[
      initializer.clone(),
      pda_account.clone(),
      system_program.clone(),
    ],
    &[&[
      payload.name.as_bytes(),
      payload.start_date.as_bytes(),
      &[bump],
    ]],
  )?;
  //
  msg!("unpacking state account");
  let mut account_data = my_try_from_slice_unchecked::<CourseState>(&pda_account.data.borrow())?;
  account_data.name = payload.name;
  account_data.degree = payload.degree;
  account_data.institution = payload.institution;
  account_data.start_date = payload.start_date;
  msg!("serializing account {:?}", account_data);
  account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;
  msg!("state account serialized");
  //
  Ok(())
}
