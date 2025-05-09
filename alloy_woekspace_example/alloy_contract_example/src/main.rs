use alloy::{
  node_bindings::Anvil,
  primitives::{Address, U256},
  providers::ProviderBuilder,
  // sol_types::sol,
  sol,
};

sol! {
    #[sol(rpc)] // <-- Important! Generates the necessary `MyContract` struct and function methods.
    #[sol(bytecode = "0x1234")] // <-- Generates the `BYTECODE` static and the `deploy` method.
    contract MyContract {
        constructor(address) {} // The `deploy` method will also include any constructor arguments.

        #[derive(Debug)]
        function doStuff(uint a, bool b) public payable returns(address c, bytes32 d);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let anvil = Anvil::new().try_spawn()?;

  // Create a provider.
  let rpc_url = anvil.endpoint().parse()?;
  let provider = ProviderBuilder::new()
    // .with_recommended_fillers()
    .connect_http(rpc_url);

  // If `#[sol(bytecode = "0x...")]` is provided, the contract can be deployed with
  // `MyContract::deploy`, and a new instance will be created.
  let constructor_arg = Address::ZERO;
  let _contract = MyContract::deploy(&provider, constructor_arg).await?;

  // Otherwise, or if already deployed, a new contract instance can be created with
  // `MyContract::new`.
  let address = Address::ZERO;
  let contract = MyContract::new(address, &provider);

  // Build a call to the `doStuff` function and configure it.
  let a = U256::from(123);
  let b = true;
  let call_builder = contract.doStuff(a, b).value(U256::from(50e18 as u64));

  // Send the call. Note that this is not broadcasted as a transaction.
  let call_return = call_builder.call().await?;
  println!("{call_return:?}"); // doStuffReturn { c: 0x..., d: 0x... }

  // Use `send` to broadcast the call as a transaction.
  let _pending_tx = call_builder.send().await?;
  Ok(())
}
