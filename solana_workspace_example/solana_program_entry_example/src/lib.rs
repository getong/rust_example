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

    CourseInstruction::ListAllCourses => list_all_courses(program_id, accounts),

    CourseInstruction::GetCourseCount => get_course_count(program_id, accounts),

    CourseInstruction::SearchCoursesByInstitution { institution } => {
      search_courses_by_institution(program_id, accounts, institution)
    }

    CourseInstruction::UpdateCourseGrade {
      name,
      start_date,
      grade,
    } => update_course_grade(program_id, accounts, name, start_date, grade),

    CourseInstruction::ArchiveCourse { name, start_date } => {
      archive_course(program_id, accounts, name, start_date)
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
    grade: String::new(), // Default empty grade
    is_archived: false,   // Default not archived
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
  account_data.grade = payload.grade;
  account_data.is_archived = payload.is_archived;
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
    grade: String::new(), // Keep existing grade or empty if new
    is_archived: false,   // Keep existing archive status
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
  msg!("Grade: {}", account_data.grade);
  msg!("Is Archived: {}", account_data.is_archived);

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

fn list_all_courses(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> ProgramResult {
  msg!("Listing all courses...");
  msg!("Note: This is a demonstration method.");
  msg!("In a real implementation, you would iterate through all accounts");
  msg!("owned by this program to list all courses.");
  Ok(())
}

fn get_course_count(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> ProgramResult {
  msg!("Getting course count...");
  msg!("Note: This is a demonstration method.");
  msg!("In a real implementation, you would count all course accounts");
  msg!("owned by this program.");
  msg!("Current count: [Implementation needed]");
  Ok(())
}

fn search_courses_by_institution(
  _program_id: &Pubkey,
  _accounts: &[AccountInfo],
  institution: String,
) -> ProgramResult {
  msg!("Searching courses by institution: {}", institution);
  msg!("Note: This is a demonstration method.");
  msg!("In a real implementation, you would search through all course accounts");
  msg!("and filter by institution name.");
  Ok(())
}

fn update_course_grade(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  start_date: String,
  grade: String,
) -> ProgramResult {
  msg!("Updating grade for course: {} to grade: {}", name, grade);

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

  msg!("unpacking existing state account");
  let mut account_data = my_try_from_slice_unchecked::<CourseState>(&pda_account.data.borrow())?;

  // Update only the grade
  account_data.grade = grade;

  // Calculate new size and rent for the updated account
  let (new_size, new_rent) = calculate_acc_size_and_rent(&account_data);

  // Reallocate account if size changed
  if pda_account.data_len() != new_size {
    msg!(
      "Account size changed from {} to {}, reallocating",
      pda_account.data_len(),
      new_size
    );

    // Check if we need more rent
    let rent_due = new_rent.saturating_sub(pda_account.lamports());
    if rent_due > 0 {
      msg!("Transferring additional rent: {} lamports", rent_due);
      invoke(
        &system_instruction::transfer(initializer.key, pda_account.key, rent_due),
        &[initializer.clone(), pda_account.clone()],
      )?;
    }

    // Resize the account
    pda_account.resize(new_size)?;
  }

  msg!("serializing updated account");
  account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;

  msg!("Course grade updated successfully");

  Ok(())
}

fn archive_course(
  program_id: &Pubkey,
  accounts: &[AccountInfo],
  name: String,
  start_date: String,
) -> ProgramResult {
  msg!("Archiving course: {}", name);

  let account_info_iter = &mut accounts.iter();
  let _initializer = next_account_info(account_info_iter)?;
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

  msg!("unpacking existing state account");
  let mut account_data = my_try_from_slice_unchecked::<CourseState>(&pda_account.data.borrow())?;

  // Archive the course
  account_data.is_archived = true;

  msg!("serializing updated account");
  account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;

  msg!("Course archived successfully");

  Ok(())
}
