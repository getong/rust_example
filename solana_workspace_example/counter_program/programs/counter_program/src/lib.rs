use anchor_lang::prelude::*;

declare_id!("AtcMC3uzVXTFHxLFZcQ78wz8Yg9kGj4a5WM9KU5BYRwT");

#[program]
pub mod counter_program {
  use super::*;
  pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    counter.authority = ctx.accounts.user.key();
    counter.count = 0;
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
  #[account(init, payer = user, space = 8 + 32 + 8)]
  pub counter: Account<'info, Counter>,
  #[account(mut)]
  pub user: Signer<'info>,
  pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct CounterOperation<'info> {
  #[account(mut, has_one = authority)]
  pub counter: Account<'info, Counter>,
  pub authority: Signer<'info>,
}
#[account]
pub struct Counter {
  pub authority: Pubkey,  // 32 bytes
  pub count: u64,         // 8 bytes
}
#[error_code]
pub enum ErrorCode {
  #[msg("Counter would underflow")]
  UnderflowError,
}
