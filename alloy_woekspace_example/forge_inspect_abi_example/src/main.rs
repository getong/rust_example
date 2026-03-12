use alloy::{
  primitives::U256,
  providers::{Provider, ProviderBuilder},
  sol,
};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Counter,
  "artifacts/Counter.sol/Counter.json"
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  MemoryExample,
  "artifacts/MemoryExample.sol/MemoryExample.json"
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  StackExample,
  "artifacts/StackExample.sol/StackExample.json"
);
sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  StorageExample,
  "artifacts/StorageExample.sol/StorageExample.json"
);

fn format_u256_list(values: &[U256]) -> String {
  let rendered = values
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(", ");
  format!("[{rendered}]")
}

#[tokio::main]
async fn main() -> Result<()> {
  let provider = ProviderBuilder::new().connect_anvil_with_wallet();
  let chain_id = provider.get_chain_id().await?;
  println!("Connected chain id: {chain_id}");
  println!();

  let counter = Counter::deploy(&provider).await?;
  let stack = StackExample::deploy(&provider).await?;
  let memory = MemoryExample::deploy(&provider).await?;
  let storage = StorageExample::deploy(&provider).await?;

  println!("Counter deployed at: {}", counter.address());
  println!("StackExample deployed at: {}", stack.address());
  println!("MemoryExample deployed at: {}", memory.address());
  println!("StorageExample deployed at: {}", storage.address());
  println!();

  let set_number_input = U256::from(42u64);
  println!("Counter::setNumber");
  println!("  input  : newNumber = {set_number_input}");
  counter.setNumber(set_number_input).send().await?.watch().await?;
  let counter_after_set = counter.number().call().await?;
  println!("  result : counter.number() = {counter_after_set}");
  println!();

  let a = U256::from(7u64);
  let b = U256::from(35u64);
  println!("StackExample::computeSum");
  println!("  input  : a = {a}, b = {b}");
  let compute_sum_result = stack.computeSum(a, b).call().await?;
  println!("  result : {compute_sum_result}");
  println!();

  let process_array_input = vec![
    U256::from(1u64),
    U256::from(2u64),
    U256::from(3u64),
    U256::from(4u64),
  ];
  println!("MemoryExample::processArray");
  println!("  input  : {}", format_u256_list(&process_array_input));
  let process_array_result = memory.processArray(process_array_input).call().await?;
  println!("  result : {}", format_u256_list(&process_array_result));
  println!();

  let initial_storage_value = U256::from(1u64);
  println!("StorageExample::storeData");
  println!("  input  : value = {initial_storage_value}");
  storage
    .storeData(initial_storage_value)
    .send()
    .await?
    .watch()
    .await?;
  let stored_before_update = storage.getData(U256::ZERO).call().await?;
  println!("  result : getData(0) = {stored_before_update}");
  println!();

  let update_index = U256::ZERO;
  let update_value = U256::from(99u64);
  println!("StorageExample::updateData");
  println!("  input  : index = {update_index}, value = {update_value}");
  storage
    .updateData(update_index, update_value)
    .send()
    .await?
    .watch()
    .await?;
  let updated_value = storage.getData(update_index).call().await?;
  let public_array_value = storage.dataArray(update_index).call().await?;
  println!("  result : getData(0) = {updated_value}");
  println!("  result : dataArray(0) = {public_array_value}");
  println!();

  println!("Counter::number");
  println!("  input  : none");
  let number_result = counter.number().call().await?;
  println!("  result : {number_result}");

  Ok(())
}
