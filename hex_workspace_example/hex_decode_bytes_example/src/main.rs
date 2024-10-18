fn main() {
  let input = "000000000000000000000000a10af672bcdd1dd61b6a63a18295e55e5f3ea842000000000000000000000000ed5fc5a4ad3e952291fe02b223b137c5d212266f0000000000000000000000000000000000000000000000001bc16d674ec8000000000000000000000000000000000000000000000000000000038d7ea4c680000000000000000000000000000000000000000000000000000000000067245590c26f8e11da9bb4e0bcc1f25044859c5a35f05a4405ce24446d5d3dc993d7899300000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000bf3a286a477967ebd850cee2dbdbfa6e535a9e6400000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000";

  let decoded = hex::decode(input).expect("Decoding failed");

  // Extracting the values based on their known positions and lengths
  let channel_id = &decoded[0 .. 32];
  let indexer = &decoded[12 .. 32];
  let consumer = &decoded[44 .. 64];
  let total = &decoded[64 .. 96];
  let price = &decoded[96 .. 128];
  let expired_at = &decoded[128 .. 160];
  let deployment_id = &decoded[160 .. 192];
  let callback = &decoded[224 .. 256];
  let _callback_extra = &decoded[256 .. 288];

  // Convert the extracted bytes into human-readable formats
  let channel_id_value = u128::from_be_bytes(
    channel_id
      .try_into()
      .expect("ChannelId slice size mismatch"),
  );
  let indexer_address = format!("0x{}", hex::encode(indexer));
  let consumer_address = format!("0x{}", hex::encode(consumer));
  let total_value = u128::from_be_bytes(total.try_into().expect("Total slice size mismatch"));
  let price_value = u128::from_be_bytes(price.try_into().expect("Price slice size mismatch"));
  let expired_at_value =
    u128::from_be_bytes(expired_at.try_into().expect("Expired slice size mismatch"));
  let deployment_id_value = format!("0x{}", hex::encode(deployment_id));
  let callback_address = format!("0x{}", hex::encode(&callback[12 .. 32]));

  // Printing the values
  println!("Channel ID: {}", channel_id_value);
  println!("Indexer Address: {}", indexer_address);
  println!("Consumer Address: {}", consumer_address);
  println!("Total: {}", total_value);
  println!("Price: {}", price_value);
  println!("Expired At (UNIX timestamp): {}", expired_at_value);
  println!("Deployment ID: {}", deployment_id_value);
  println!("Callback Address: {}", callback_address);
}
