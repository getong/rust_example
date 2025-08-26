use sui_sdk::SuiClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
  let sui = SuiClientBuilder::default()
    .build("http://127.0.0.1:9000") // local network address
    .await?;
  println!("Sui local network version: {}", sui.api_version());

  // local Sui network, like the above one but using the dedicated function
  let sui_local = SuiClientBuilder::default().build_localnet().await?;
  println!("Sui local network version: {}", sui_local.api_version());

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
