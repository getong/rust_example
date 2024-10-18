use ethers::contract::EthEvent;
use ethers::core::types::{Address, Bytes, H256, U256};
use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Clone, Debug, EthEvent, Serialize, Deserialize)]
struct ChannelOpen {
  #[ethevent(indexed)]
  channelId: U256,
  indexer: Address,
  consumer: Address,
  total: U256,
  price: U256,
  expiredAt: U256,
  deploymentId: H256,
  callback: Bytes,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
  // Hex string from the log data (replace with actual log data)
let hex_string = ["0x000000000000000000000000a10af672bcdd1dd61b6a63a18295e55e5f3ea842000000000000000000000000ed5fc5a4ad3e952291fe02b223b137c5d212266f0000000000000000000000000000000000000000000000001bc16d674ec8000000000000000000000000000000000000000000000000000000038d7ea4c680000000000000000000000000000000000000000000000000000000000067245590c26f8e11da9bb4e0bcc1f25044859c5a35f05a4405ce24446d5d3dc993d7899300000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000bf3a286a477967ebd850cee2dbdbfa6e535a9e6400000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000",
    "0x000000000000000000000000a10af672bcdd1dd61b6a63a18295e55e5f3ea842000000000000000000000000ed5fc5a4ad3e952291fe02b223b137c5d212266f0000000000000000000000000000000000000000000000001bc16d674ec8000000000000000000000000000000000000000000000000000000038d7ea4c680000000000000000000000000000000000000000000000000000000000067244f98c26f8e11da9bb4e0bcc1f25044859c5a35f05a4405ce24446d5d3dc993d7899300000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000bf3a286a477967ebd850cee2dbdbfa6e535a9e6400000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000"
];

for i in hex_string
      let bytes = Bytes::from_static(i.as_bytes());
      let channel_open = decode_hex_to_struct(&bytes.as_ref()).await;
      println!("i : {}\nchannel_open: {:#?}\n", i, channel_open);

  Ok(())

async fn decode_hex_to_struct(decoded_bytes: &[u8]) -> ChannelOpen {
  // Manually slice and decode the fields from the byte array
  let channel_id = U256::from_big_endian(&decoded_bytes[0 .. 32]);
  let indexer = Address::from_slice(&decoded_bytes[12 .. 32]); // First 20 bytes of the 32
  let consumer = Address::from_slice(&decoded_bytes[44 .. 64]); // Next 20 bytes of the 32
  let total = U256::from_big_endian(&decoded_bytes[64 .. 96]);
  let price = U256::from_big_endian(&decoded_bytes[96 .. 128]);
  let expired_at = U256::from_big_endian(&decoded_bytes[128 .. 160]);
  let deployment_id = H256::from_slice(&decoded_bytes[160 .. 192]);

  // Extract callback, in this case, it's the remaining bytes
  let callback = Bytes::from(decoded_bytes[224 ..].to_vec());

  // Create the `ChannelOpen` struct
  ChannelOpen {
    channelId: channel_id,
    indexer,
    consumer,
    total,
    price,
    expiredAt: expired_at,
    deploymentId: deployment_id,
    callback,
  }
}
