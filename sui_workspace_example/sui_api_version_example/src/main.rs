use sui_sdk::SuiClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
  // Sui testnet -- https://fullnode.testnet.sui.io:443
  let sui_testnet = SuiClientBuilder::default().build_testnet().await?;
  println!("Sui testnet version: {}", sui_testnet.api_version());

  // Sui devnet -- https://fullnode.devnet.sui.io:443
  let sui_devnet = SuiClientBuilder::default().build_devnet().await?;
  println!("Sui devnet version: {}", sui_devnet.api_version());

  // Sui mainnet -- https://fullnode.mainnet.sui.io:443
  let sui_mainnet = SuiClientBuilder::default().build_mainnet().await?;
  println!("Sui mainnet version: {}", sui_mainnet.api_version());

  Ok(())
}
