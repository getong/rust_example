use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

// Data layout for the integer account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct IntegerAccount {
    pub value: i64,
}

// Program instructions
pub enum IntegerInstruction {
    /// Initialize account with initial value
    /// Accounts: [writable] data_account, [signer] payer
    Initialize { value: i64 },
    /// Add value to account
    /// Accounts: [writable] data_account
    Add { value: i64 },
    /// Subtract value from account
    /// Accounts: [writable] data_account
    Minus { value: i64 },
    /// Divide account by value (panics if zero)
    /// Accounts: [writable] data_account
    Divide { value: i64 },
}

impl IntegerInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        // Very basic unpacking, for demonstration
        // [0]=tag, [1..9]=i64 little endian
        if input.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let tag = input[0];
        if input.len() < 9 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let value = i64::from_le_bytes(input[1..9].try_into().unwrap());
        match tag {
            0 => Ok(Self::Initialize { value }),
            1 => Ok(Self::Add { value }),
            2 => Ok(Self::Minus { value }),
            3 => Ok(Self::Divide { value }),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = IntegerInstruction::unpack(instruction_data)?;
    let accounts_iter = &mut accounts.iter();

    match instruction {
        IntegerInstruction::Initialize { value } => {
            let data_account = next_account_info(accounts_iter)?;
            // Here you typically check owner, rent, etc. Skipped for brevity
            let mut int_data =
                IntegerAccount::try_from_slice(&data_account.data.borrow()) // Try to deserialize
                .unwrap_or(IntegerAccount { value: 0 }); // Or create if uninitialized
            int_data.value = value;
            int_data.serialize(&mut &mut data_account.data.borrow_mut()[..])?;
            msg!("Initialized with value: {}", value);
        }
        IntegerInstruction::Add { value } => {
            let data_account = next_account_info(accounts_iter)?;
            let mut int_data = IntegerAccount::try_from_slice(&data_account.data.borrow())?;
            int_data.value += value;
            int_data.serialize(&mut &mut data_account.data.borrow_mut()[..])?;
            msg!("Added {}, new value: {}", value, int_data.value);
        }
        IntegerInstruction::Minus { value } => {
            let data_account = next_account_info(accounts_iter)?;
            let mut int_data = IntegerAccount::try_from_slice(&data_account.data.borrow())?;
            int_data.value -= value;
            int_data.serialize(&mut &mut data_account.data.borrow_mut()[..])?;
            msg!("Subtracted {}, new value: {}", value, int_data.value);
        }
        IntegerInstruction::Divide { value } => {
            let data_account = next_account_info(accounts_iter)?;
            if value == 0 {
                msg!("Division by zero!");
                return Err(ProgramError::InvalidInstructionData);
            }
            let mut int_data = IntegerAccount::try_from_slice(&data_account.data.borrow())?;
            int_data.value /= value;
            int_data.serialize(&mut &mut data_account.data.borrow_mut()[..])?;
            msg!("Divided by {}, new value: {}", value, int_data.value);
        }
    }
    Ok(())
}
