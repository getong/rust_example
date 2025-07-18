use borsh::BorshSerialize;
use solana_program::{
  account_info::{next_account_info, AccountInfo},
  entrypoint,
  entrypoint::ProgramResult,
  msg,
  program::{invoke, invoke_signed},
  program_error::ProgramError,
  pubkey::Pubkey,
};
use solana_system_interface::instruction as system_instruction;

pub mod misc;

use misc::{
  calculate_acc_size_and_rent, derive_pda_address, derive_pda_from_name_and_date,
  my_try_from_slice_unchecked, CourseInstruction, CourseState,
};

entrypoint!(process_instruction);

fn process_instruction(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  instruction_data: &[u8],
) -> ProgramResult {
  msg!("start execution...");

  let instruction = CourseInstruction::unpack(instruction_data)?;

  match instruction {
    CourseInstruction::AddCourse {
      name,
      degree,
      institution,
      start_date,
    } => add_course(program_id, accounts, name, degree, institution, start_date),

    CourseInstruction::UpdateCourse {
      name,
      degree,
      institution,
      start_date,
    } => update_course(program_id, accounts, name, degree, institution, start_date),

    CourseInstruction::ReadCourse { name, start_date } => {
      read_course(program_id, accounts, name, start_date)
    }

    CourseInstruction::DeleteCourse { name, start_date } => {
      delete_course(program_id, accounts, name, start_date)
    }
  }
}

fn add_course(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  degree: String,
  institution: String,
  start_date: String,
) -> ProgramResult {
  msg!("Adding course: {}", name);

  let account_info_iter = &mut accounts.iter();
  let initializer = next_account_info(account_info_iter)?;
  let pda_account = next_account_info(account_info_iter)?;
  let system_program = next_account_info(account_info_iter)?;

  let payload = CourseState {
    name: name.clone(),
    degree: degree.clone(),
    institution: institution.clone(),
    start_date: start_date.clone(),
  };

  let (pda, bump) = derive_pda_address(&payload, program_id);
  let (size, rent) = calculate_acc_size_and_rent(&payload);

  if *pda_account.key != pda {
    return Err(ProgramError::InvalidArgument);
  }

  msg!("pda account and key identical");

  // Check if account already exists
  if pda_account.data_len() > 0 {
    msg!("Course already exists!");
    return Err(ProgramError::AccountAlreadyInitialized);
  }

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

  msg!("unpacking state account");
  let mut account_data = my_try_from_slice_unchecked::<CourseState>(&pda_account.data.borrow())?;
  account_data.name = payload.name;
  account_data.degree = payload.degree;
  account_data.institution = payload.institution;
  account_data.start_date = payload.start_date;
  msg!("serializing account {:?}", account_data);
  account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;
  msg!("Course added successfully");

  Ok(())
}

fn update_course(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  degree: String,
  institution: String,
  start_date: String,
) -> ProgramResult {
  msg!("Updating course: {}", name);

  let account_info_iter = &mut accounts.iter();
  let initializer = next_account_info(account_info_iter)?;
  let pda_account = next_account_info(account_info_iter)?;
  let system_program = next_account_info(account_info_iter)?;

  let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, program_id);

  if *pda_account.key != pda {
    return Err(ProgramError::InvalidArgument);
  }

  // Check if account exists
  if pda_account.data_len() == 0 {
    msg!("Course does not exist!");
    return Err(ProgramError::UninitializedAccount);
  }

  let updated_payload = CourseState {
    name: name.clone(),
    degree: degree.clone(),
    institution: institution.clone(),
    start_date: start_date.clone(),
  };

  let (new_size, new_rent) = calculate_acc_size_and_rent(&updated_payload);

  // Reallocate account if size changed
  if pda_account.data_len() != new_size {
    let rent_due = new_rent.saturating_sub(pda_account.lamports());
    if rent_due > 0 {
      invoke(
        &system_instruction::transfer(initializer.key, pda_account.key, rent_due),
        &[
          initializer.clone(),
          pda_account.clone(),
          system_program.clone(),
        ],
      )?;
    }
    pda_account.resize(new_size)?;
  }

  msg!("unpacking existing state account");
  let mut account_data = pda_account.try_borrow_mut_data()?;

  // Update the fields
  updated_payload.serialize(&mut &mut account_data[..])?;

  msg!("Course updated successfully");

  Ok(())
}

fn read_course(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  start_date: String,
) -> ProgramResult {
  msg!("Reading course: {}", name);

  let account_info_iter = &mut accounts.iter();
  let pda_account = next_account_info(account_info_iter)?;

  let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, program_id);

  if *pda_account.key != pda {
    return Err(ProgramError::InvalidArgument);
  }

  // Check if account exists
  if pda_account.data_len() == 0 {
    msg!("Course does not exist!");
    return Err(ProgramError::UninitializedAccount);
  }

  msg!("unpacking state account");
  let account_data = my_try_from_slice_unchecked::<CourseState>(&pda_account.data.borrow())?;

  msg!("Course details:");
  msg!("Name: {}", account_data.name);
  msg!("Degree: {}", account_data.degree);
  msg!("Institution: {}", account_data.institution);
  msg!("Start Date: {}", account_data.start_date);

  Ok(())
}

fn delete_course(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  start_date: String,
) -> ProgramResult {
  msg!("Deleting course: {}", name);

  let account_info_iter = &mut accounts.iter();
  let initializer = next_account_info(account_info_iter)?;
  let pda_account = next_account_info(account_info_iter)?;

  let (pda, _bump) = derive_pda_from_name_and_date(&name, &start_date, program_id);

  if *pda_account.key != pda {
    return Err(ProgramError::InvalidArgument);
  }

  // Check if account exists
  if pda_account.data_len() == 0 {
    msg!("Course does not exist!");
    return Err(ProgramError::UninitializedAccount);
  }

  // Transfer lamports back to initializer
  let dest_starting_lamports = initializer.lamports();
  **initializer.lamports.borrow_mut() = dest_starting_lamports
    .checked_add(pda_account.lamports())
    .ok_or(ProgramError::ArithmeticOverflow)?;
  **pda_account.lamports.borrow_mut() = 0;

  // Clear the account data
  let mut data = pda_account.data.borrow_mut();
  data.fill(0);

  msg!("Course deleted successfully");

  Ok(())
}
