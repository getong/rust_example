use anchor_lang::prelude::*;

declare_id!("DLrjrBhue2NFMdzhSPPjGV2noUEaNWN9asJY6rJZ3Eyh");

const COUNTER_SEED: &[u8] = b"counter";

#[program]
pub mod multi_counter_program {
  use super::*;

  pub fn increment(ctx: Context<CounterOperation>, id: u64) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    ensure_initialized(counter, ctx.accounts.user.key(), id)?;
    counter.count = counter
      .count
      .checked_add(1)
      .ok_or(ErrorCode::OverflowError)?;
    Ok(())
  }

  pub fn decrement(ctx: Context<CounterOperation>, id: u64) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    if ensure_initialized(counter, ctx.accounts.user.key(), id)? {
      // First touch creates the account and leaves count at 0.
      return Ok(());
    }
    counter.count = counter
      .count
      .checked_sub(1)
      .ok_or(ErrorCode::UnderflowError)?;
    Ok(())
  }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CounterOperation<'info> {
  #[account(
        init_if_needed,
        payer = user,
        space = 8 + Counter::INIT_SPACE,
        seeds = [COUNTER_SEED, user.key().as_ref(), &id.to_le_bytes()],
        bump
    )]
  pub counter: Account<'info, Counter>,
  #[account(mut)]
  pub user: Signer<'info>,
  pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct Counter {
  pub owner: Pubkey,
  pub id: u64,
  pub count: u64,
  pub initialized: bool,
}

#[error_code]
pub enum ErrorCode {
  #[msg("Counter would overflow")]
  OverflowError,
  #[msg("Counter would underflow")]
  UnderflowError,
  #[msg("Counter does not belong to this user")]
  CounterOwnershipMismatch,
  #[msg("Counter id mismatch")]
  CounterIdMismatch,
}

fn ensure_initialized(counter: &mut Account<Counter>, user: Pubkey, id: u64) -> Result<bool> {
  if !counter.initialized {
    counter.owner = user;
    counter.id = id;
    counter.count = 0;
    counter.initialized = true;
    return Ok(true);
  }

  require_keys_eq!(counter.owner, user, ErrorCode::CounterOwnershipMismatch);
  require_eq!(counter.id, id, ErrorCode::CounterIdMismatch);
  Ok(false)
}
