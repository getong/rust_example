# Uniswap V4 SDK Rust Example

This example demonstrates how to use the Uniswap V4 SDK for Rust to interact with Uniswap V4 pools.

## Features

- Pool information retrieval
- Swap quote calculation
- Position management examples

## Setup

1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Update the RPC_URL in `.env` with your Ethereum RPC endpoint (e.g., from Alchemy or Infura)

3. Install dependencies and run:
   ```bash
   cargo run
   ```

## Examples Included

### 1. Pool Information
- Creating token instances
- Displaying token pair information

### 2. Swap Quote
- Setting up tokens for swapping
- Calculating swap amounts
- Configuring fee tiers

### 3. Position Management
- Setting up liquidity positions
- Defining tick ranges
- Managing liquidity amounts

## Dependencies

- `uniswap-v4-sdk`: The main SDK for Uniswap V4
- `uniswap-sdk-core`: Core SDK utilities
- `uniswap-v3-sdk`: V3 SDK for compatibility
- `alloy`: Ethereum interaction library
- `tokio`: Async runtime
- `eyre`: Error handling
- `dotenv`: Environment variable management

## Important Notes

- The example uses default test addresses for WETH and USDC on Ethereum mainnet
- The private key in `.env.example` is a well-known test key - DO NOT use it with real funds
- Always use a testnet or local fork for development and testing