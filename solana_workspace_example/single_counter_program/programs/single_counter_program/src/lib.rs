use anchor_lang::prelude::*;

declare_id!("Df95mfc4tfPChpmk5kGPFTyynNZk4RmtiXpDufCTuQ6m");

const COUNTER_SEED: &[u8] = b"counter";

#[program]
pub mod counter_program {
  use super::*;

  pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;

    // Keep initialize idempotent: only set defaults on first creation.
    if !counter.initialized {
      counter.count = 0;
      counter.initialized = true;
    }

    Ok(())
  }

  pub fn increment(ctx: Context<CounterOperation>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    counter.count += 1;
    Ok(())
  }

  pub fn decrement(ctx: Context<CounterOperation>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    counter.count = counter.count.checked_sub(1).ok_or(ErrorCode::UnderflowError)?;
    Ok(())
  }
}
#[derive(Accounts)]
pub struct Initialize<'info> {
  #[account(
    init_if_needed,
    payer = user,
    space = 8 + 8 + 1,
    seeds = [COUNTER_SEED],
    bump
  )]
  pub counter: Account<'info, Counter>,
  #[account(mut)]
  pub user: Signer<'info>,
  pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CounterOperation<'info> {
  #[account(mut, seeds = [COUNTER_SEED], bump)]
  pub counter: Account<'info, Counter>,
  pub user: Signer<'info>,
}

#[account]
pub struct Counter {
  pub count: u64,
  pub initialized: bool,
}

#[error_code]
pub enum ErrorCode {
  #[msg("Counter would underflow")]
  UnderflowError,
}
