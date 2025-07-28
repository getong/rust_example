use std::env;

use alloy::{
  network::EthereumWallet, primitives::U256, providers::ProviderBuilder,
  signers::local::PrivateKeySigner,
};
use dotenv::dotenv;
use eyre::Result;

mod abi;
mod compound;
use compound::CompoundV3;

#[tokio::main]
async fn main() -> Result<()> {
  dotenv().ok();

  let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set");
  let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());

  let signer: PrivateKeySigner = private_key.parse()?;
  let wallet = EthereumWallet::from(signer);

  let provider = ProviderBuilder::new()
    .wallet(wallet)
    .connect_http(rpc_url.parse()?);

  let compound = CompoundV3::new(provider);

  println!("=== Compound V3 Example ===\n");

  println!("Fetching market info...");
  let market_info = compound.get_market_info().await?;
  println!("Market Info:");
  println!(
    "  Total Supply: {} USDC",
    market_info.total_supply / U256::from(1_000_000)
  );
  println!(
    "  Total Borrow: {} USDC",
    market_info.total_borrow / U256::from(1_000_000)
  );
  println!(
    "  Utilization: {}%",
    market_info.utilization * U256::from(100) / U256::from(1e18)
  );
  println!(
    "  Supply APR: {:.2}%",
    market_info.supply_rate as f64 * 365.25 * 24.0 * 60.0 * 60.0 / 1e18 * 100.0
  );
  println!(
    "  Borrow APR: {:.2}%\n",
    market_info.borrow_rate as f64 * 365.25 * 24.0 * 60.0 * 60.0 / 1e18 * 100.0
  );

  println!("Fetching account info...");
  let account_info = compound.get_account_info().await?;
  println!("Account Info:");
  println!(
    "  USDC Balance: {} USDC",
    account_info.usdc_balance / U256::from(1_000_000)
  );
  println!(
    "  Supplied Balance: {} USDC",
    account_info.supplied_balance / U256::from(1_000_000)
  );
  println!(
    "  Borrow Balance: {} USDC\n",
    account_info.borrow_balance / U256::from(1_000_000)
  );

  let action = env::var("ACTION").unwrap_or_else(|_| "info".to_string());

  match action.as_str() {
    "supply" => {
      let amount = env::var("AMOUNT")
        .expect("AMOUNT must be set for supply action")
        .parse::<f64>()
        .expect("AMOUNT must be a valid number");

      let usdc_amount = U256::from((amount * 1_000_000.0) as u64);

      println!("Supplying {} USDC to Compound v3...", amount);
      let supply_tx = compound.supply_usdc(usdc_amount).await?;
      println!("Supply transaction: 0x{:x}", supply_tx.transaction_hash);
      println!("Gas used: {}", supply_tx.gas_used);
    }
    "withdraw" => {
      let amount = env::var("AMOUNT")
        .expect("AMOUNT must be set for withdraw action")
        .parse::<f64>()
        .expect("AMOUNT must be a valid number");

      let usdc_amount = U256::from((amount * 1_000_000.0) as u64);

      println!("Withdrawing {} USDC from Compound v3...", amount);
      let withdraw_tx = compound.withdraw_usdc(usdc_amount).await?;
      println!("Withdraw transaction: 0x{:x}", withdraw_tx.transaction_hash);
      println!("Gas used: {}", withdraw_tx.gas_used);
    }
    "borrow" => {
      let amount = env::var("AMOUNT")
        .expect("AMOUNT must be set for borrow action")
        .parse::<f64>()
        .expect("AMOUNT must be a valid number");

      let borrow_amount = U256::from((amount * 1_000_000.0) as u64);

      println!("Borrowing {} USDC from Compound v3...", amount);
      let borrow_tx = compound.borrow_base(borrow_amount).await?;
      println!("Borrow transaction: 0x{:x}", borrow_tx.transaction_hash);
      println!("Gas used: {}", borrow_tx.gas_used);
    }
    "repay" => {
      let amount = env::var("AMOUNT")
        .expect("AMOUNT must be set for repay action")
        .parse::<f64>()
        .expect("AMOUNT must be a valid number");

      let repay_amount = U256::from((amount * 1_000_000.0) as u64);

      println!("Repaying {} USDC to Compound v3...", amount);
      let repay_tx = compound.repay_base(repay_amount).await?;
      println!("Repay transaction: 0x{:x}", repay_tx.transaction_hash);
      println!("Gas used: {}", repay_tx.gas_used);
    }
    _ => {
      println!(
        "No action specified. Set ACTION env var to 'supply', 'withdraw', 'borrow', or 'repay'"
      );
    }
  }

  Ok(())
}
