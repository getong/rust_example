use alloy::{
    primitives::{Address, U256},
    providers::{builder, Provider},
};
use eyre::Result;
use foundry_contracts::counter::Counter;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a provider with an Anvil testnet and an unlocked wallet
    let provider = builder().with_recommended_fillers().on_anvil_with_wallet();

    // Deploy the Counter contract
    println!("Deploying Counter contract...");
    let address = "0x0000000000000000000000000000000000000000".parse::<Address>()?;
    let counter = Counter::new(address, provider.clone());
    println!("Counter deployed at: {:?}", counter.address());

    // Wait a bit for the deployment to be mined
    sleep(Duration::from_secs(1)).await;

    // Convert 42 to `U256`
    let value = U256::from(42);

    // Call `setNumber(42)`
    let pending_tx = counter.setNumber(value).send().await?;
    println!("setNumber(42) transaction sent: {:?}", pending_tx.tx_hash());

    // Wait for transaction confirmation
    let receipt = pending_tx.get_receipt().await?;
    println!("Transaction confirmed in block: {:?}", receipt.block_number);

    // Call `increment()`
    let pending_tx = counter.increment().send().await?;
    println!("setNumber(42) transaction sent: {:?}", pending_tx.tx_hash());

    // Wait for transaction confirmation
    let receipt = pending_tx.get_receipt().await?;
    println!("Transaction confirmed in block: {:?}", receipt.block_number);

    // Call `number()` to fetch the updated value
    let current_number = counter.number().call().await?;
    println!("Counter value: {}", current_number._0);

    // Fetch the current block number
    let blk = provider.get_block_number().await?;
    println!("Current block number: {}", blk);

    Ok(())
}
