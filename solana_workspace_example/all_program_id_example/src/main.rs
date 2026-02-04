use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_program::pubkey::Pubkey;

fn main() {
  let rpc_url = String::from("https://api.devnet.solana.com");
  let connection = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

  let program_id = Pubkey::from_str("6a2GdmttJdanBkoHt7f4Kon4hfadx4UTUgJeRkCaiL3U").unwrap();
  let accounts = connection.get_program_accounts(&program_id).unwrap();

  println!("accounts for {}, {:?}", program_id, accounts);
}
