use ethers::{
  core::{
    abi::AbiDecode,
    types::{Address, BlockNumber, Filter, U256},
  },
  providers::{Middleware, Provider, StreamExt, Ws},
};
use eyre::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
  let client =
    Provider::<Ws>::connect("wss://mainnet.infura.io/ws/v3/c60b0bb42f8a4c6481ecd229eddaca27")
      .await?;
  let client = Arc::new(client);

  let last_block = client
    .get_block(BlockNumber::Latest)
    .await?
    .unwrap()
    .number
    .unwrap();
  println!("last_block: {last_block}");

  let erc20_transfer_filter = Filter::new()
    .from_block(last_block - 25)
    .event("Transfer(address,address,uint256)");

  let mut stream = client.subscribe_logs(&erc20_transfer_filter).await?.take(2);

  while let Some(log) = stream.next().await {
    println!(
      "block: {:?}, tx: {:?}, token: {:?}, from: {:?}, to: {:?}, amount: {:?}",
      log.block_number,
      log.transaction_hash,
      log.address,
      Address::from(log.topics[1]),
      Address::from(log.topics[2]),
      U256::decode(log.data)
    );
  }

  Ok(())
}

/*
last_block: 21011384
block: Some(21011359), tx: Some(0xeda2adfed4cdae3e5525eb0a386dd23248e77d230ba8316d6da00748a0872cf5), token: 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2, from: 0x1f2f10d1c40777ae1da742455c65828ff36df387, to: 0x52c77b0cb827afbad022e6d6caf2c44452edbc39, amount: Ok(19243454729497346048)
block: Some(21011359), tx: Some(0xeda2adfed4cdae3e5525eb0a386dd23248e77d230ba8316d6da00748a0872cf5), token: 0xe0f63a424a4439cbe457d80e4f4b51ad25b2c56c, from: 0x52c77b0cb827afbad022e6d6caf2c44452edbc39, to: 0x1f2f10d1c40777ae1da742455c65828ff36df387, amount: Ok(6112330711040)
*/
// code copy from https://www.gakonst.com/ethers-rs/subscriptions/logs.html
