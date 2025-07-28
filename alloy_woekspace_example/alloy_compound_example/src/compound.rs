use alloy::{
  primitives::{Address, U256},
  providers::Provider,
};
use eyre::Result;

use crate::abi::{IComet, IERC20};

pub struct CompoundV3<P: Provider> {
  provider: P,
  comet_address: Address,
  usdc_address: Address,
}

impl<P: Provider> CompoundV3<P> {
  pub fn new(provider: P) -> Self {
    let comet_address = "0xc3d688B66703497DAA19211EEdff47f25384cdc3"
      .parse()
      .unwrap();
    let usdc_address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
      .parse()
      .unwrap();

    Self {
      provider,
      comet_address,
      usdc_address,
    }
  }

  pub async fn approve_usdc(&self, amount: U256) -> Result<alloy::rpc::types::TransactionReceipt> {
    let usdc = IERC20::new(self.usdc_address, &self.provider);

    let approve_call = usdc.approve(self.comet_address, amount);
    let pending_tx = approve_call.send().await?;
    let receipt = pending_tx.get_receipt().await?;

    Ok(receipt)
  }

  pub async fn supply_usdc(&self, amount: U256) -> Result<alloy::rpc::types::TransactionReceipt> {
    self.approve_usdc(amount).await?;

    let comet = IComet::new(self.comet_address, &self.provider);
    let supply_call = comet.supply(self.usdc_address, amount);
    let pending_tx = supply_call.send().await?;
    let receipt = pending_tx.get_receipt().await?;

    Ok(receipt)
  }

  pub async fn withdraw_usdc(&self, amount: U256) -> Result<alloy::rpc::types::TransactionReceipt> {
    let comet = IComet::new(self.comet_address, &self.provider);
    let withdraw_call = comet.withdraw(self.usdc_address, amount);
    let pending_tx = withdraw_call.send().await?;
    let receipt = pending_tx.get_receipt().await?;

    Ok(receipt)
  }

  pub async fn get_account_info(&self) -> Result<AccountInfo> {
    let comet = IComet::new(self.comet_address, &self.provider);
    let usdc = IERC20::new(self.usdc_address, &self.provider);

    let wallet_address = self.provider.get_accounts().await?[0];

    let supplied_balance = comet.balanceOf(wallet_address).call().await?;
    let borrow_balance = comet.borrowBalanceOf(wallet_address).call().await?;
    let usdc_balance = usdc.balanceOf(wallet_address).call().await?;

    Ok(AccountInfo {
      supplied_balance,
      borrow_balance,
      usdc_balance,
    })
  }

  pub async fn borrow_base(&self, amount: U256) -> Result<alloy::rpc::types::TransactionReceipt> {
    let comet = IComet::new(self.comet_address, &self.provider);
    let base_token = comet.baseToken().call().await?;

    let withdraw_call = comet.withdraw(base_token, amount);
    let pending_tx = withdraw_call.send().await?;
    let receipt = pending_tx.get_receipt().await?;

    Ok(receipt)
  }

  pub async fn repay_base(&self, amount: U256) -> Result<alloy::rpc::types::TransactionReceipt> {
    let comet = IComet::new(self.comet_address, &self.provider);
    let base_token = comet.baseToken().call().await?;

    let base_token_contract = IERC20::new(base_token, &self.provider);
    let approve_call = base_token_contract.approve(self.comet_address, amount);
    approve_call.send().await?.get_receipt().await?;

    let supply_call = comet.supply(base_token, amount);
    let pending_tx = supply_call.send().await?;
    let receipt = pending_tx.get_receipt().await?;

    Ok(receipt)
  }

  pub async fn get_market_info(&self) -> Result<MarketInfo> {
    let comet = IComet::new(self.comet_address, &self.provider);

    let total_supply = comet.totalSupply().call().await?;
    let total_borrow = comet.totalBorrow().call().await?;
    let utilization = comet.getUtilization().call().await?;
    let supply_rate = comet.getSupplyRate(utilization).call().await?;
    let borrow_rate = comet.getBorrowRate(utilization).call().await?;

    Ok(MarketInfo {
      total_supply,
      total_borrow,
      utilization,
      supply_rate,
      borrow_rate,
    })
  }
}

#[derive(Debug)]
pub struct AccountInfo {
  pub supplied_balance: U256,
  pub borrow_balance: U256,
  pub usdc_balance: U256,
}

#[derive(Debug)]
pub struct MarketInfo {
  pub total_supply: U256,
  pub total_borrow: U256,
  pub utilization: U256,
  pub supply_rate: u64,
  pub borrow_rate: u64,
}
