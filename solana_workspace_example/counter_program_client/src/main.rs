use std::{convert::TryInto, env, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::{Keypair, read_keypair_file, write_keypair_file};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::from_str_const("HFLzm7QPRkbboJYszHyHGkbuSbjgHQuVUvdXJwpiwU8c");
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::from_str_const("11111111111111111111111111111111");
const INIT_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const INC_DISCRIMINATOR: [u8; 8] = [11, 18, 104, 9, 104, 174, 59, 33];
const DEC_DISCRIMINATOR: [u8; 8] = [106, 227, 168, 59, 248, 27, 150, 101];
const COUNTER_ACCOUNT_DISCRIMINATOR: [u8; 8] = [255, 176, 4, 245, 188, 253, 124, 25];

fn main() -> Result<()> {
  let rpc_url = env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".to_string());
  let wallet_path =
    env::var("SOLANA_KEYPAIR").unwrap_or_else(|_| "~/.config/solana/id.json".to_string());
  let counter_keypair_path =
    env::var("COUNTER_KEYPAIR").unwrap_or_else(|_| "./counter-keypair.json".to_string());

  let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
  let payer = read_keypair_file(expand_tilde(&wallet_path))
    .map_err(|e| anyhow!("failed to read payer keypair: {wallet_path}: {e}"))?;

  let args: Vec<String> = env::args().collect();
  let cmd = args.get(1).map(String::as_str).unwrap_or("show");

  match cmd {
    "init" => {
      let (counter, needs_save) = load_or_generate_counter_keypair(&counter_keypair_path)?;
      let sig = initialize(&rpc, &payer, &counter, PROGRAM_ID)?;
      if needs_save {
        save_counter_keypair(&counter, &counter_keypair_path)?;
      }
      println!("initialize tx: {sig}");
      print_counter(&rpc, &counter.pubkey())?;
    }
    "inc" => {
      let counter = read_counter_keypair(&counter_keypair_path)?;
      let sig = increment(&rpc, &payer, &counter.pubkey(), PROGRAM_ID)?;
      println!("increment tx: {sig}");
      print_counter(&rpc, &counter.pubkey())?;
    }
    "dec" => {
      let counter = read_counter_keypair(&counter_keypair_path)?;
      let sig = decrement(&rpc, &payer, &counter.pubkey(), PROGRAM_ID)?;
      println!("decrement tx: {sig}");
      print_counter(&rpc, &counter.pubkey())?;
    }
    "show" => {
      let counter = read_counter_keypair(&counter_keypair_path)?;
      print_counter(&rpc, &counter.pubkey())?;
    }
    _ => print_usage(),
  }

  Ok(())
}

fn initialize(
  rpc: &RpcClient,
  payer: &Keypair,
  counter: &Keypair,
  program_id: Pubkey,
) -> Result<String> {
  let ix = Instruction {
    program_id,
    accounts: vec![
      AccountMeta::new(counter.pubkey(), true),
      AccountMeta::new(payer.pubkey(), true),
      AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
    ],
    data: INIT_DISCRIMINATOR.to_vec(),
  };
  send_tx(rpc, payer, &[ix], vec![payer, counter])
}

fn increment(
  rpc: &RpcClient,
  payer: &Keypair,
  counter: &Pubkey,
  program_id: Pubkey,
) -> Result<String> {
  let ix = Instruction {
    program_id,
    accounts: vec![
      AccountMeta::new(*counter, false),
      AccountMeta::new_readonly(payer.pubkey(), true),
    ],
    data: INC_DISCRIMINATOR.to_vec(),
  };
  send_tx(rpc, payer, &[ix], vec![payer])
}

fn decrement(
  rpc: &RpcClient,
  payer: &Keypair,
  counter: &Pubkey,
  program_id: Pubkey,
) -> Result<String> {
  let ix = Instruction {
    program_id,
    accounts: vec![
      AccountMeta::new(*counter, false),
      AccountMeta::new_readonly(payer.pubkey(), true),
    ],
    data: DEC_DISCRIMINATOR.to_vec(),
  };
  send_tx(rpc, payer, &[ix], vec![payer])
}

fn send_tx(
  rpc: &RpcClient,
  payer: &Keypair,
  instructions: &[Instruction],
  signers: Vec<&Keypair>,
) -> Result<String> {
  let recent_blockhash = rpc
    .get_latest_blockhash()
    .context("failed to fetch latest blockhash")?;
  let tx = Transaction::new_signed_with_payer(
    instructions,
    Some(&payer.pubkey()),
    &signers,
    recent_blockhash,
  );
  let sig = rpc
    .send_and_confirm_transaction(&tx)
    .context("failed to send/confirm transaction")?;
  Ok(sig.to_string())
}

fn print_counter(rpc: &RpcClient, counter_pubkey: &Pubkey) -> Result<()> {
  let data = rpc
    .get_account_data(counter_pubkey)
    .with_context(|| format!("failed to fetch counter account: {counter_pubkey}"))?;
  let (authority, count) = decode_counter_account(&data)?;
  println!("counter account: {counter_pubkey}");
  println!("authority: {authority}");
  println!("count: {count}");
  Ok(())
}

fn decode_counter_account(data: &[u8]) -> Result<(Pubkey, u64)> {
  if data.len() < 48 {
    bail!("counter account data too short: {}", data.len());
  }
  if data[.. 8] != COUNTER_ACCOUNT_DISCRIMINATOR {
    bail!("unexpected account discriminator");
  }
  let authority_bytes: [u8; 32] = data[8 .. 40]
    .try_into()
    .map_err(|_| anyhow!("invalid authority bytes"))?;
  let authority = Pubkey::from(authority_bytes);
  let count = u64::from_le_bytes(
    data[40 .. 48]
      .try_into()
      .map_err(|_| anyhow!("invalid counter bytes"))?,
  );
  Ok((authority, count))
}

fn load_or_generate_counter_keypair(path: &str) -> Result<(Keypair, bool)> {
  let path_buf = expand_tilde(path);
  if path_buf.exists() {
    let kp = read_keypair_file(&path_buf)
      .map_err(|e| anyhow!("failed to read counter keypair {}: {e}", path_buf.display()))?;
    return Ok((kp, false));
  }
  Ok((Keypair::new(), true))
}

fn save_counter_keypair(kp: &Keypair, path: &str) -> Result<()> {
  let path_buf = expand_tilde(path);
  write_keypair_file(kp, &path_buf)
    .map_err(|e| anyhow!(e.to_string()))
    .with_context(|| format!("failed to write counter keypair: {}", path_buf.display()))?;
  Ok(())
}

fn read_counter_keypair(path: &str) -> Result<Keypair> {
  let path_buf = expand_tilde(path);
  if !path_buf.exists() {
    bail!(
      "counter keypair not found: {}. Run `cargo run -- init` first",
      path_buf.display()
    );
  }
  read_keypair_file(&path_buf)
    .map_err(|e| anyhow!("failed to read counter keypair {}: {e}", path_buf.display()))
}

fn expand_tilde(path: &str) -> PathBuf {
  if let Some(stripped) = path.strip_prefix("~/") {
    if let Some(home) = env::var_os("HOME") {
      return PathBuf::from(home).join(stripped);
    }
  }
  PathBuf::from(path)
}

fn print_usage() {
  eprintln!("Usage:");
  eprintln!("  cargo run -- init   # initialize counter account");
  eprintln!("  cargo run -- inc    # increment counter");
  eprintln!("  cargo run -- dec    # decrement counter");
  eprintln!("  cargo run -- show   # read counter account");
  eprintln!();
  eprintln!("Environment variables:");
  eprintln!("  SOLANA_RPC_URL   (default: http://127.0.0.1:8899)");
  eprintln!("  SOLANA_KEYPAIR   (default: ~/.config/solana/id.json)");
  eprintln!("  COUNTER_KEYPAIR  (default: ./counter-keypair.json)");
}
