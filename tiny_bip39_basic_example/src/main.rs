use anyhow::Result;
// https://crates.io/crates/tiny-bip39
use bip39::{Language, Mnemonic, MnemonicType};

fn main() -> Result<()> {
  let mnemonic = Mnemonic::new(MnemonicType::Words24, Language::English);
  let phrase = mnemonic.phrase();

  println!("phrase: {}", phrase);
  Ok(())
}
