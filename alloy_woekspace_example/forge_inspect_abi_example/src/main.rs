use std::{error::Error, io::Error as IoError};

use alloy::{
  json_abi::JsonAbi,
  primitives::{U256, hex},
  sol,
  sol_types::SolCall,
};

const COUNTER_ABI_JSON: &str = include_str!("../Counter.abi.json");
const MEMORY_ABI_JSON: &str = include_str!("../MemoryExample.abi.json");
const STACK_ABI_JSON: &str = include_str!("../StackExample.abi.json");
const STORAGE_ABI_JSON: &str = include_str!("../StorageExample.abi.json");

sol!(
  CounterAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/Counter.abi.json")
);
sol!(
  MemoryExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/MemoryExample.abi.json")
);
sol!(
  StackExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/StackExample.abi.json")
);
sol!(
  StorageExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/StorageExample.abi.json")
);

fn load_abi(contract_name: &str, raw_json: &str) -> Result<JsonAbi, Box<dyn Error>> {
  serde_json::from_str(raw_json).map_err(|err| {
    IoError::other(format!("failed to parse {contract_name} ABI JSON: {err}")).into()
  })
}

fn print_abi_summary(contract_name: &str, abi: &JsonAbi) {
  println!("{contract_name} ABI -> recovered Solidity interface:");
  println!("{}", abi.to_sol(&format!("I{contract_name}"), None));
  println!();
}

fn print_source_comparison_notes() {
  println!("Source comparison notes:");
  println!("  ABI JSON only contains callable ABI items, not the original contract body.");
  println!(
    "  `uint256 public number;` and `uint256[] public dataArray;` become getter functions in ABI."
  );
  println!(
    "  `public/external` and `memory/calldata` from the source may be normalized in ABI output."
  );
  println!();
}

fn main() -> Result<(), Box<dyn Error>> {
  let counter_abi = load_abi("Counter", COUNTER_ABI_JSON)?;
  let memory_abi = load_abi("MemoryExample", MEMORY_ABI_JSON)?;
  let stack_abi = load_abi("StackExample", STACK_ABI_JSON)?;
  let storage_abi = load_abi("StorageExample", STORAGE_ABI_JSON)?;

  print_abi_summary("Counter", &counter_abi);
  print_abi_summary("MemoryExample", &memory_abi);
  print_abi_summary("StackExample", &stack_abi);
  print_abi_summary("StorageExample", &storage_abi);
  print_source_comparison_notes();

  let set_number_call = CounterAbi::setNumberCall {
    newNumber: U256::from(42u64),
  };
  println!("CounterAbi::setNumberCall");
  println!("  signature: {}", CounterAbi::setNumberCall::SIGNATURE);
  println!(
    "  selector : 0x{}",
    hex::encode(CounterAbi::setNumberCall::SELECTOR)
  );
  println!(
    "  calldata : 0x{}",
    hex::encode(set_number_call.abi_encode())
  );
  println!();

  let compute_sum_call = StackExampleAbi::computeSumCall {
    a: U256::from(7u64),
    b: U256::from(35u64),
  };
  println!("StackExampleAbi::computeSumCall");
  println!(
    "  signature: {}",
    StackExampleAbi::computeSumCall::SIGNATURE
  );
  println!(
    "  selector : 0x{}",
    hex::encode(StackExampleAbi::computeSumCall::SELECTOR)
  );
  println!(
    "  calldata : 0x{}",
    hex::encode(compute_sum_call.abi_encode())
  );
  let compute_sum_return = StackExampleAbi::computeSumCall::abi_decode_returns(
    &StackExampleAbi::computeSumCall::abi_encode_returns(&U256::from(42u64)),
  )?;
  println!("  decoded mocked return: {compute_sum_return}");
  println!();

  let process_array_call = MemoryExampleAbi::processArrayCall {
    input: vec![
      U256::from(1u64),
      U256::from(2u64),
      U256::from(3u64),
      U256::from(4u64),
    ],
  };
  println!("MemoryExampleAbi::processArrayCall");
  println!(
    "  signature: {}",
    MemoryExampleAbi::processArrayCall::SIGNATURE
  );
  println!(
    "  selector : 0x{}",
    hex::encode(MemoryExampleAbi::processArrayCall::SELECTOR)
  );
  println!(
    "  calldata : 0x{}",
    hex::encode(process_array_call.abi_encode())
  );
  let process_array_return = MemoryExampleAbi::processArrayCall::abi_decode_returns(
    &MemoryExampleAbi::processArrayCall::abi_encode_returns(&vec![
      U256::from(2u64),
      U256::from(4u64),
      U256::from(6u64),
      U256::from(8u64),
    ]),
  )?;
  println!("  decoded mocked return: {:?}", process_array_return);
  println!();

  let update_data_call = StorageExampleAbi::updateDataCall {
    index: U256::from(0u64),
    value: U256::from(99u64),
  };
  println!("StorageExampleAbi::updateDataCall");
  println!(
    "  signature: {}",
    StorageExampleAbi::updateDataCall::SIGNATURE
  );
  println!(
    "  selector : 0x{}",
    hex::encode(StorageExampleAbi::updateDataCall::SELECTOR)
  );
  println!(
    "  calldata : 0x{}",
    hex::encode(update_data_call.abi_encode())
  );
  println!();

  let number_call_return = CounterAbi::numberCall::abi_decode_returns(
    &CounterAbi::numberCall::abi_encode_returns(&U256::from(42u64)),
  )?;
  println!("CounterAbi::numberCall mocked return: {number_call_return}");

  Ok(())
}
