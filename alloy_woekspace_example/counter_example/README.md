# counter_example

`counter_example` demonstrates deploying and interacting with the `Counter` contract (from `config_example/src/Counter.sol`) using:

- Rust
- `alloy`
- local `anvil`

This example uses the compiled artifact:

- `abi/Counter.json` (copied from `config_example/out/Counter.sol/Counter.json`)

## What it does

1. Starts a local Anvil node.
2. Deploys `Counter` with constructor value `11`.
3. Calls:
   - `number()`
   - `setNumber(42)`
   - `increment()`
   - `resetToNetworkDefault()`
4. Asserts the reset value is `7` on Anvil (`chainId = 31337`), matching `CounterChainConfig`.

## Run

From workspace root:

```bash
cargo run -p counter_example
```
