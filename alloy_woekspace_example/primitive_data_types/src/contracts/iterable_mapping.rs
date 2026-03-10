use std::str::FromStr;

use alloy::{
  network::{ReceiptResponse, TransactionBuilder},
  primitives::{Bytes, U256},
  providers::Provider,
  rpc::types::TransactionRequest,
  sol,
};
use eyre::{Result, ensure};

const TEST_ITERABLE_MAP_BYTECODE: &str = include_str!("../../abi/TestIterableMapBytecode.json");

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  IterableMapping,
  "abi/IterableMapping.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestIterableMap,
  "abi/TestIterableMap.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let caller = provider
    .get_accounts()
    .await?
    .first()
    .copied()
    .ok_or_else(|| eyre::eyre!("no unlocked account available from provider"))?;

  let library = IterableMapping::deploy(provider).await?;
  let linked_bytecode = link_test_iterable_map_bytecode(library.address())?;
  let receipt = provider
    .send_transaction(TransactionRequest::default().with_deploy_code(linked_bytecode))
    .await?
    .get_receipt()
    .await?;
  let contract_address = receipt
    .contract_address()
    .ok_or_else(|| eyre::eyre!("TestIterableMap deployment receipt missing contract address"))?;
  let contract = TestIterableMap::new(contract_address, provider);

  println!(
    "[IterableMapping] library: {}, contract: {}",
    library.address(),
    contract.address()
  );

  contract
    .setInMapping(U256::from(55_u64))
    .send()
    .await?
    .watch()
    .await?;
  let size_after_set = contract.sizeOfMapping().call().await?;
  let value = contract.getFromMap().call().await?;
  let first_key = contract.getKeyAtIndex(U256::ZERO).call().await?;
  ensure!(
    first_key == caller,
    "expected first iterable-map key to equal caller"
  );

  contract.removeFromMapping().send().await?.watch().await?;
  let size_after_remove = contract.sizeOfMapping().call().await?;
  println!(
    "[IterableMapping] value={value}, size_after_set={size_after_set}, \
     size_after_remove={size_after_remove}"
  );
  Ok(())
}

fn link_test_iterable_map_bytecode(library_address: &alloy::primitives::Address) -> Result<Bytes> {
  let mut linked = TEST_ITERABLE_MAP_BYTECODE.trim().to_string();
  let address_hex = library_address
    .to_string()
    .trim_start_matches("0x")
    .to_ascii_lowercase();

  while let Some(start) = linked.find("__$") {
    let marker_end = linked[start ..]
      .find("$__")
      .ok_or_else(|| eyre::eyre!("found library placeholder start without closing marker"))?;
    linked.replace_range(start .. start + marker_end + 3, &address_hex);
  }

  Ok(Bytes::from_str(&linked)?)
}
