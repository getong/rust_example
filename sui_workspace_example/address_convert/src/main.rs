use std::str::FromStr;

use sui_sdk::types::base_types::SuiAddress;

#[tokio::main]
async fn main() {
  println!("Sui Address Converter using Sui SDK\n");

  // Example addresses
  let test_addresses = vec![
    "0x1",
    "0x2",
    "0xa",
    "0xff",
    "0x100",
    "0x1234",
    "0xCAFE",
    "0xdeadbeef",
    "0x0000000000000000000000000000000000000000000000000000000000000001",
  ];

  for addr_str in test_addresses {
    println!("Original: {}", addr_str);

    match SuiAddress::from_str(addr_str) {
      Ok(sui_address) => {
        println!("Parsed SuiAddress: {}", sui_address);
        println!("Normalized: {}", sui_address.to_string());

        // Get the inner bytes and display them
        let bytes = sui_address.to_inner();
        println!("Bytes: {:?}", bytes);

        // Convert bytes back to address
        let from_bytes = SuiAddress::try_from(bytes.as_slice());
        match from_bytes {
          Ok(reconstructed) => println!("Reconstructed: {}", reconstructed),
          Err(e) => println!("Error reconstructing: {}", e),
        }
      }
      Err(e) => println!("Error parsing address: {}", e),
    }
    println!("---");
  }

  // Demonstrate special addresses
  println!("\nSpecial Addresses:");
  if let Ok(sui_framework) = SuiAddress::from_str("0x2") {
    println!("Sui Framework: {}", sui_framework);
  }
  if let Ok(std_lib) = SuiAddress::from_str("0x1") {
    println!("Standard Library: {}", std_lib);
  }

  // Generate random address
  let random_address = SuiAddress::random_for_testing_only();
  println!("Random address: {}", random_address);
}
