use std::{env, error::Error, io::Error as IoError, str::FromStr};

use alloy::{
  json_abi::JsonAbi,
  primitives::{Address, U256, hex},
  providers::{Provider, ProviderBuilder},
  sol,
  sol_types::SolCall,
};

const COUNTER_ABI_JSON: &str = include_str!("../Counter.abi.json");
const MEMORY_ABI_JSON: &str = include_str!("../MemoryExample.abi.json");
const STACK_ABI_JSON: &str = include_str!("../StackExample.abi.json");
const STORAGE_ABI_JSON: &str = include_str!("../StorageExample.abi.json");

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  CounterAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/Counter.abi.json")
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  MemoryExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/MemoryExample.abi.json")
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  StackExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/StackExample.abi.json")
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  StorageExampleAbi,
  concat!(env!("CARGO_MANIFEST_DIR"), "/StorageExample.abi.json")
);

#[derive(Debug)]
struct RuntimeConfig {
  rpc_url: String,
  counter_address: Option<Address>,
  memory_address: Option<Address>,
  stack_address: Option<Address>,
  storage_address: Option<Address>,
}

impl RuntimeConfig {
  fn from_env_and_args() -> Result<Self, Box<dyn Error>> {
    let mut config = Self {
      rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_owned()),
      counter_address: read_optional_address_env("COUNTER_ADDRESS")?,
      memory_address: read_optional_address_env("MEMORY_EXAMPLE_ADDRESS")?,
      stack_address: read_optional_address_env("STACK_EXAMPLE_ADDRESS")?,
      storage_address: read_optional_address_env("STORAGE_EXAMPLE_ADDRESS")?,
    };

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
      match arg.as_str() {
        "--rpc-url" => config.rpc_url = next_arg_value(&mut args, "--rpc-url")?,
        "--counter-address" => {
          config.counter_address = Some(parse_address(
            "--counter-address",
            &next_arg_value(&mut args, "--counter-address")?,
          )?)
        }
        "--memory-address" => {
          config.memory_address = Some(parse_address(
            "--memory-address",
            &next_arg_value(&mut args, "--memory-address")?,
          )?)
        }
        "--stack-address" => {
          config.stack_address = Some(parse_address(
            "--stack-address",
            &next_arg_value(&mut args, "--stack-address")?,
          )?)
        }
        "--storage-address" => {
          config.storage_address = Some(parse_address(
            "--storage-address",
            &next_arg_value(&mut args, "--storage-address")?,
          )?)
        }
        "--help" | "-h" => {
          print_usage();
          std::process::exit(0);
        }
        other => {
          return Err(IoError::other(format!("unsupported argument: {other}")).into());
        }
      }
    }

    Ok(config)
  }

  fn has_any_contract_address(&self) -> bool {
    self.counter_address.is_some()
      || self.memory_address.is_some()
      || self.stack_address.is_some()
      || self.storage_address.is_some()
  }
}

fn next_arg_value(
  args: &mut impl Iterator<Item = String>,
  flag: &str,
) -> Result<String, Box<dyn Error>> {
  args
    .next()
    .ok_or_else(|| IoError::other(format!("missing value for {flag}")).into())
}

fn parse_address(label: &str, value: &str) -> Result<Address, Box<dyn Error>> {
  Address::from_str(value).map_err(|err| {
    IoError::other(format!(
      "failed to parse {label} `{value}` as address: {err}"
    ))
    .into()
  })
}

fn read_optional_address_env(var_name: &str) -> Result<Option<Address>, Box<dyn Error>> {
  match env::var(var_name) {
    Ok(value) => Ok(Some(parse_address(var_name, &value)?)),
    Err(env::VarError::NotPresent) => Ok(None),
    Err(err) => Err(IoError::other(format!("failed to read {var_name}: {err}")).into()),
  }
}

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

fn print_runtime_config(config: &RuntimeConfig) {
  println!("Runtime configuration:");
  println!("  rpc_url               = {}", config.rpc_url);
  println!(
    "  counter_address       = {}",
    format_optional_address(config.counter_address)
  );
  println!(
    "  memory_example_address = {}",
    format_optional_address(config.memory_address)
  );
  println!(
    "  stack_example_address  = {}",
    format_optional_address(config.stack_address)
  );
  println!(
    "  storage_example_address = {}",
    format_optional_address(config.storage_address)
  );
  println!();
}

fn format_optional_address(address: Option<Address>) -> String {
  address
    .map(|value| value.to_string())
    .unwrap_or_else(|| "<not provided>".to_owned())
}

fn print_usage() {
  println!("Usage:");
  println!(
    "  cargo run -- [--rpc-url <url>] [--counter-address <0x...>] [--memory-address <0x...>] \
     [--stack-address <0x...>] [--storage-address <0x...>]"
  );
  println!();
  println!("Environment variables:");
  println!("  RPC_URL");
  println!("  COUNTER_ADDRESS");
  println!("  MEMORY_EXAMPLE_ADDRESS");
  println!("  STACK_EXAMPLE_ADDRESS");
  println!("  STORAGE_EXAMPLE_ADDRESS");
}

async fn print_onchain_examples<P: Provider>(
  provider: &P,
  config: &RuntimeConfig,
) -> Result<(), Box<dyn Error>> {
  let chain_id = provider.get_chain_id().await?;
  println!("Connected chain id: {chain_id}");
  println!();

  if let Some(address) = config.counter_address {
    let counter = CounterAbi::new(address, provider);
    println!("CounterAbi via provider");
    println!("  contract address: {}", counter.address());
    let number = counter.number().call().await?;
    println!("  number() -> {number}");
    println!();
  }

  if let Some(address) = config.stack_address {
    let stack = StackExampleAbi::new(address, provider);
    let sum = stack
      .computeSum(U256::from(7u64), U256::from(35u64))
      .call()
      .await?;
    println!("StackExampleAbi via provider");
    println!("  contract address: {}", stack.address());
    println!("  computeSum(7, 35) -> {sum}");
    println!();
  }

  if let Some(address) = config.memory_address {
    let memory = MemoryExampleAbi::new(address, provider);
    let processed = memory
      .processArray(vec![
        U256::from(1u64),
        U256::from(2u64),
        U256::from(3u64),
        U256::from(4u64),
      ])
      .call()
      .await?;
    println!("MemoryExampleAbi via provider");
    println!("  contract address: {}", memory.address());
    println!("  processArray([1, 2, 3, 4]) -> {:?}", processed);
    println!();
  }

  if let Some(address) = config.storage_address {
    let storage = StorageExampleAbi::new(address, provider);
    let first_value = storage.getData(U256::ZERO).call().await?;
    let public_array_value = storage.dataArray(U256::ZERO).call().await?;
    println!("StorageExampleAbi via provider");
    println!("  contract address: {}", storage.address());
    println!("  getData(0) -> {first_value}");
    println!("  dataArray(0) -> {public_array_value}");
    println!();
  }

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let config = RuntimeConfig::from_env_and_args()?;

  let counter_abi = load_abi("Counter", COUNTER_ABI_JSON)?;
  let memory_abi = load_abi("MemoryExample", MEMORY_ABI_JSON)?;
  let stack_abi = load_abi("StackExample", STACK_ABI_JSON)?;
  let storage_abi = load_abi("StorageExample", STORAGE_ABI_JSON)?;

  print_runtime_config(&config);
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
  println!();

  if config.has_any_contract_address() {
    let provider = ProviderBuilder::new().connect_http(config.rpc_url.parse()?);
    print_onchain_examples(&provider, &config).await?;
  } else {
    println!("No contract addresses were provided, so on-chain provider calls were skipped.");
    println!(
      "Pass --rpc-url plus one or more --*-address flags to execute eth_call through the provider."
    );
  }

  Ok(())
}
