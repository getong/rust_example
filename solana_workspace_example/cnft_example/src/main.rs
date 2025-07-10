use std::str::FromStr;

use mpl_bubblegum::{
  instructions::{MintToCollectionV1, MintToCollectionV1InstructionArgs},
  types::{Collection, Creator, MetadataArgs, TokenProgramVersion, TokenStandard},
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
  commitment_config::CommitmentConfig,
  message::Message,
  pubkey::Pubkey,
  signature::{Signer, read_keypair_file},
  transaction::Transaction,
};

// Helper function to derive tree authority PDA
fn derive_tree_authority(merkle_tree: &Pubkey) -> (Pubkey, u8) {
  Pubkey::find_program_address(&[merkle_tree.as_ref()], &mpl_bubblegum::ID)
}

// Helper function to derive metadata PDA
fn derive_metadata_pda(mint: &Pubkey) -> (Pubkey, u8) {
  Pubkey::find_program_address(
    &[
      "metadata".as_bytes(),
      mpl_token_metadata::ID.as_ref(),
      mint.as_ref(),
    ],
    &mpl_token_metadata::ID,
  )
}

// Helper function to derive edition PDA
fn derive_edition_pda(mint: &Pubkey) -> (Pubkey, u8) {
  Pubkey::find_program_address(
    &[
      "metadata".as_bytes(),
      mpl_token_metadata::ID.as_ref(),
      mint.as_ref(),
      "edition".as_bytes(),
    ],
    &mpl_token_metadata::ID,
  )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Connect to localhost devnet
  let rpc = RpcClient::new_with_commitment(
    "http://127.0.0.1:8899".to_string(),
    CommitmentConfig::confirmed(),
  );

  // Load your wallet (expand the ~ path)
  let home_dir = std::env::var("HOME").unwrap();
  let keypair_path = format!("{}/.config/solana/id.json", home_dir);
  let payer = read_keypair_file(&keypair_path)?;

  // Example addresses - Replace these with actual values
  // For testing, you can create these using the Solana CLI or other tools
  let merkle_tree = Pubkey::from_str("11111111111111111111111111111112")?; // Replace with actual merkle tree
  let collection_mint = Pubkey::from_str("11111111111111111111111111111113")?; // Replace with actual collection mint

  // Derive required PDAs
  let (tree_authority, _) = derive_tree_authority(&merkle_tree);
  let (collection_metadata, _) = derive_metadata_pda(&collection_mint);
  let (collection_edition, _) = derive_edition_pda(&collection_mint);

  // Update authority (must match collection NFT's update authority)
  // let _collection_update_authority = payer.pubkey();

  // Leaf owner (who gets the NFT)
  let leaf_owner = payer.pubkey();

  // Metadata for NFT
  let name = "My Compressed NFT".to_string();
  let symbol = "CNFT".to_string();
  let uri = "https://arweave.net/your_metadata.json".to_string();

  println!("Creating compressed NFT...");
  println!("Merkle Tree: {}", merkle_tree);
  println!("Collection Mint: {}", collection_mint);
  println!("Tree Authority: {}", tree_authority);
  println!("Collection Metadata: {}", collection_metadata);
  println!("Collection Edition: {}", collection_edition);

  // Build the instruction using the correct API
  let ix = MintToCollectionV1 {
    tree_config: merkle_tree,
    leaf_owner,
    leaf_delegate: payer.pubkey(),
    merkle_tree,
    payer: payer.pubkey(),
    tree_creator_or_delegate: payer.pubkey(),
    collection_authority: payer.pubkey(),
    collection_authority_record_pda: None,
    collection_mint,
    collection_metadata,
    collection_edition,
    bubblegum_signer: tree_authority,
    log_wrapper: spl_noop::ID,
    compression_program: spl_account_compression::ID,
    token_metadata_program: mpl_token_metadata::ID,
    system_program: solana_sdk::system_program::ID,
  }
  .instruction(MintToCollectionV1InstructionArgs {
    metadata: MetadataArgs {
      name,
      symbol,
      uri,
      creators: vec![Creator {
        address: payer.pubkey(),
        verified: true,
        share: 100,
      }],
      seller_fee_basis_points: 500, // 5%
      primary_sale_happened: false,
      is_mutable: true,
      edition_nonce: None,
      token_standard: Some(TokenStandard::NonFungible),
      collection: Some(Collection {
        verified: true,
        key: collection_mint,
      }),
      uses: None,
      token_program_version: TokenProgramVersion::Original,
    },
  });

  // Build the transaction
  let message = Message::new(&[ix], Some(&payer.pubkey()));
  let blockhash = rpc.get_latest_blockhash()?;
  let tx = Transaction::new(&[&payer], message, blockhash);

  // Send transaction
  println!("Sending transaction...");
  match rpc.send_and_confirm_transaction(&tx) {
    Ok(sig) => {
      println!("✅ Successfully minted compressed NFT!");
      println!("Transaction signature: {}", sig);
    }
    Err(e) => {
      println!("❌ Failed to mint compressed NFT: {}", e);
      return Err(e.into());
    }
  }

  Ok(())
}
