use solana_client::rpc_client::RpcClient;
use solana_sdk::{
  commitment_config::CommitmentConfig,
  native_token::LAMPORTS_PER_SOL,
  signature::{Keypair, Signer},
};
use solana_system_interface::instruction as system_instruction;

fn main() {
  // Create an HTTP RpcClient with specified "confirmed" commitment level
  // "confirmed" - the node will query the most recent block that has been voted on by supermajority
  // of the cluster.
  let rpc_url = String::from("https://api.devnet.solana.com");
  let rpc_client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

  // Generate fee payer and new account key pairs
  let fee_payer = Keypair::new();
  let new_account = Keypair::new();

  // Request an airdrop for the fee payer and wait for the transaction to be confirmed
  let request_airdrop_tx_signature = rpc_client
    .request_airdrop(&fee_payer.pubkey(), LAMPORTS_PER_SOL)
    .unwrap();
  loop {
    if let Ok(confirmed) = rpc_client.confirm_transaction(&request_airdrop_tx_signature) {
      if confirmed {
        break;
      }
    }
  }

  // Specify account data length
  let space = 0;
  // Get minimum balance required to make an account with specified data length rent exempt
  let rent_exemption_amount = rpc_client
    .get_minimum_balance_for_rent_exemption(space)
    .unwrap();
  // Create instruction to create an account
  let create_account_ix = system_instruction::create_account(
    &fee_payer.pubkey(),
    &new_account.pubkey(),
    rent_exemption_amount,
    space as u64,
    &fee_payer.pubkey(),
  );

  // Get recent blockhash
  let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
  // Create transaction to create an account
  let create_account_tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
    &[create_account_ix],
    Some(&fee_payer.pubkey()),
    &[&fee_payer, &new_account],
    recent_blockhash,
  );

  // Submit a transaction to create an account and wait for confirmation
  let create_account_tx_signature = rpc_client
    .send_and_confirm_transaction(&create_account_tx)
    .unwrap();

  // Print transaction signature and account address
  println!("Transaction signature: {create_account_tx_signature}");
  println!("New account {} created successfully", new_account.pubkey());
}

// copy from https://medium.com/@maicmi/solana-basic-cheat-sheet-cb32c732c26e
