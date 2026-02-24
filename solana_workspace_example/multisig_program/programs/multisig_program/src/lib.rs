use anchor_lang::prelude::*;

declare_id!("3Q8QU44JzRbXgYeDuTEYEiyNGDDqkXNHKc2Ex4QoeC4o");

#[program]
pub mod multisig_program {
  use super::*;

  pub fn create_multisig(
    ctx: Context<CreateMultisig>,
    owners: Vec<Pubkey>,
    threshold: u64,
  ) -> Result<()> {
    require!(!owners.is_empty(), ErrorCode::EmptyOwners);
    require!(
      owners.len() <= Multisig::MAX_OWNERS,
      ErrorCode::TooManyOwners
    );
    require!(
      threshold > 0 && threshold <= owners.len() as u64,
      ErrorCode::InvalidThreshold
    );
    require!(has_no_duplicate_owners(&owners), ErrorCode::DuplicateOwner);

    let multisig = &mut ctx.accounts.multisig;
    multisig.owners = owners;
    multisig.threshold = threshold;
    multisig.owner_set_seqno = 0;
    Ok(())
  }

  pub fn create_transaction(
    ctx: Context<CreateTransaction>,
    program_id: Pubkey,
    accounts: Vec<TransactionAccountMeta>,
    data: Vec<u8>,
  ) -> Result<()> {
    let multisig = &ctx.accounts.multisig;
    let proposer = ctx.accounts.proposer.key();

    require!(
      accounts.len() <= Transaction::MAX_ACCOUNTS,
      ErrorCode::TooManyAccounts
    );
    require!(
      data.len() <= Transaction::MAX_DATA_LEN,
      ErrorCode::InstructionDataTooLarge
    );

    let proposer_index = owner_index(multisig, proposer)?;
    let transaction = &mut ctx.accounts.transaction;
    transaction.multisig = multisig.key();
    transaction.proposer = proposer;
    transaction.program_id = program_id;
    transaction.accounts = accounts;
    transaction.data = data;
    transaction.signers = vec![false; multisig.owners.len()];
    transaction.signers[proposer_index] = true;
    transaction.did_execute = false;
    transaction.owner_set_seqno = multisig.owner_set_seqno;
    Ok(())
  }

  pub fn approve(ctx: Context<Approve>) -> Result<()> {
    let multisig = &ctx.accounts.multisig;
    let owner = ctx.accounts.owner.key();
    let transaction = &mut ctx.accounts.transaction;

    require!(
      !transaction.did_execute,
      ErrorCode::TransactionAlreadyExecuted
    );
    require!(
      transaction.owner_set_seqno == multisig.owner_set_seqno,
      ErrorCode::OwnerSetChanged
    );

    let owner_idx = owner_index(multisig, owner)?;
    require!(!transaction.signers[owner_idx], ErrorCode::AlreadyApproved);
    transaction.signers[owner_idx] = true;
    Ok(())
  }

  pub fn revoke(ctx: Context<Approve>) -> Result<()> {
    let multisig = &ctx.accounts.multisig;
    let owner = ctx.accounts.owner.key();
    let transaction = &mut ctx.accounts.transaction;

    require!(
      !transaction.did_execute,
      ErrorCode::TransactionAlreadyExecuted
    );
    require!(
      transaction.owner_set_seqno == multisig.owner_set_seqno,
      ErrorCode::OwnerSetChanged
    );

    let owner_idx = owner_index(multisig, owner)?;
    require!(transaction.signers[owner_idx], ErrorCode::NotApprovedYet);
    transaction.signers[owner_idx] = false;
    Ok(())
  }

  pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
    let multisig = &ctx.accounts.multisig;
    let transaction = &mut ctx.accounts.transaction;

    require!(
      !transaction.did_execute,
      ErrorCode::TransactionAlreadyExecuted
    );
    require!(
      transaction.owner_set_seqno == multisig.owner_set_seqno,
      ErrorCode::OwnerSetChanged
    );
    require!(
      transaction.multisig == multisig.key(),
      ErrorCode::InvalidMultisig
    );
    require!(
      transaction.signers.len() == multisig.owners.len(),
      ErrorCode::InvalidSignersState
    );

    let signed = transaction.signers.iter().filter(|flag| **flag).count() as u64;
    require!(signed >= multisig.threshold, ErrorCode::NotEnoughSigners);

    // Simplified example: only threshold check + executed flag, no CPI call.
    transaction.did_execute = true;
    msg!(
      "Transaction approved and marked executed. target_program={}",
      transaction.program_id
    );
    Ok(())
  }
}

#[derive(Accounts)]
pub struct CreateMultisig<'info> {
  #[account(init, payer = payer, space = Multisig::INIT_SPACE)]
  pub multisig: Account<'info, Multisig>,
  #[account(mut)]
  pub payer: Signer<'info>,
  pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateTransaction<'info> {
  #[account(mut)]
  pub multisig: Account<'info, Multisig>,
  #[account(init, payer = proposer, space = Transaction::INIT_SPACE)]
  pub transaction: Account<'info, Transaction>,
  #[account(mut)]
  pub proposer: Signer<'info>,
  pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Approve<'info> {
  pub multisig: Account<'info, Multisig>,
  #[account(
        mut,
        constraint = transaction.multisig == multisig.key() @ ErrorCode::InvalidMultisig
    )]
  pub transaction: Account<'info, Transaction>,
  pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
  pub multisig: Account<'info, Multisig>,
  #[account(
        mut,
        constraint = transaction.multisig == multisig.key() @ ErrorCode::InvalidMultisig
    )]
  pub transaction: Account<'info, Transaction>,
  pub executor: Signer<'info>,
}

#[account]
pub struct Multisig {
  pub owners: Vec<Pubkey>,
  pub threshold: u64,
  pub owner_set_seqno: u64,
}

impl Multisig {
  pub const MAX_OWNERS: usize = 10;
  pub const INIT_SPACE: usize = 8 + 4 + (Self::MAX_OWNERS * 32) + 8 + 8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TransactionAccountMeta {
  pub pubkey: Pubkey,
  pub is_signer: bool,
  pub is_writable: bool,
}

#[account]
pub struct Transaction {
  pub multisig: Pubkey,
  pub proposer: Pubkey,
  pub program_id: Pubkey,
  pub accounts: Vec<TransactionAccountMeta>,
  pub data: Vec<u8>,
  pub signers: Vec<bool>,
  pub did_execute: bool,
  pub owner_set_seqno: u64,
}

impl Transaction {
  pub const MAX_ACCOUNTS: usize = 16;
  pub const MAX_DATA_LEN: usize = 512;
  pub const INIT_SPACE: usize = 8
    + 32
    + 32
    + 32
    + 4
    + (Self::MAX_ACCOUNTS * (32 + 1 + 1))
    + 4
    + Self::MAX_DATA_LEN
    + 4
    + Multisig::MAX_OWNERS
    + 1
    + 8;
}

fn owner_index(multisig: &Multisig, owner: Pubkey) -> Result<usize> {
  multisig
    .owners
    .iter()
    .position(|current| *current == owner)
    .ok_or(error!(ErrorCode::InvalidOwner))
}

fn has_no_duplicate_owners(owners: &[Pubkey]) -> bool {
  for (idx, owner) in owners.iter().enumerate() {
    if owners[idx + 1..].contains(owner) {
      return false;
    }
  }
  true
}

#[error_code]
pub enum ErrorCode {
  #[msg("The owners list cannot be empty")]
  EmptyOwners,
  #[msg("Threshold must be between 1 and owners length")]
  InvalidThreshold,
  #[msg("Too many owners")]
  TooManyOwners,
  #[msg("Owner list has duplicates")]
  DuplicateOwner,
  #[msg("Signer is not an owner")]
  InvalidOwner,
  #[msg("Transaction has already been executed")]
  TransactionAlreadyExecuted,
  #[msg("Signer already approved this transaction")]
  AlreadyApproved,
  #[msg("Signer has not approved this transaction yet")]
  NotApprovedYet,
  #[msg("Not enough signers")]
  NotEnoughSigners,
  #[msg("Owner set changed after transaction creation")]
  OwnerSetChanged,
  #[msg("Too many accounts in instruction")]
  TooManyAccounts,
  #[msg("Instruction data too large")]
  InstructionDataTooLarge,
  #[msg("Transaction does not belong to this multisig")]
  InvalidMultisig,
  #[msg("Signer bitmap does not match owner set")]
  InvalidSignersState,
}
