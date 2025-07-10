#!/bin/bash

# Compressed NFT Setup Script
echo "🌳 Setting up Compressed NFT Example..."

# Check if Solana CLI is installed
if ! command -v solana &> /dev/null; then
    echo "❌ Solana CLI not found. Installing..."
    sh -c "$(curl -sSfL https://release.solana.com/v1.18.4/install)"
    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
fi

# Configure for devnet
echo "🔧 Configuring Solana CLI for devnet..."
solana config set --url https://api.devnet.solana.com

# Check if keypair exists
if [ ! -f "$HOME/.config/solana/id.json" ]; then
    echo "🔑 Generating new keypair..."
    solana-keygen new --outfile "$HOME/.config/solana/id.json"
fi

# Get wallet address
WALLET=$(solana address)
echo "💰 Wallet address: $WALLET"

# Check SOL balance
BALANCE=$(solana balance | cut -d' ' -f1)
echo "💰 Current balance: $BALANCE SOL"

# Airdrop SOL if balance is low
if (( $(echo "$BALANCE < 1" | bc -l) )); then
    echo "💸 Requesting airdrop..."
    solana airdrop 2
else
    echo "✅ Sufficient SOL balance"
fi

# Install Sugar CLI for creating trees and collections
if ! command -v sugar &> /dev/null; then
    echo "🍭 Installing Sugar CLI..."
    bash <(curl -sSf https://sugar.metaplex.com/install.sh)
fi

echo ""
echo "✅ Setup complete!"
echo ""
echo "Next steps:"
echo "1. Create a Merkle tree: sugar create-tree --max-depth 20 --max-buffer-size 64"
echo "2. Create a collection NFT: sugar create-collection"
echo "3. Update the addresses in src/main.rs"
echo "4. Upload your metadata to Arweave/IPFS"
echo "5. Run: cargo run"
echo ""
echo "📚 See README.md for detailed instructions"
