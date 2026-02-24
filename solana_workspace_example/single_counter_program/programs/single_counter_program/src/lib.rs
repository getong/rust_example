use anchor_lang::prelude::*;

declare_id!("Df95mfc4tfPChpmk5kGPFTyynNZk4RmtiXpDufCTuQ6m");

const COUNTER_SEED: &[u8] = b"counter";

#[program]
pub mod counter_program {
  use super::*;

  pub fn increment(ctx: Context<CounterOperation>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    ensure_initialized(counter);
    counter.count = counter.count.checked_add(1).ok_or(ErrorCode::OverflowError)?;
    Ok(())
  }

  pub fn decrement(ctx: Context<CounterOperation>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    // If this is the first touch, initialize and treat decrement as a no-op.
    if ensure_initialized(counter) {
      return Ok(());
    }
    counter.count = counter.count.checked_sub(1).ok_or(ErrorCode::UnderflowError)?;
    Ok(())
  }
}

#[derive(Accounts)]
pub struct CounterOperation<'info> {
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

#[account]
pub struct Counter {
  pub count: u64,
  pub initialized: bool,
}

#[error_code]
pub enum ErrorCode {
  #[msg("Counter would overflow")]
  OverflowError,
  #[msg("Counter would underflow")]
  UnderflowError,
}

fn ensure_initialized(counter: &mut Account<Counter>) -> bool {
  if !counter.initialized {
    counter.count = 0;
    counter.initialized = true;
    return true;
  }

  false
}
