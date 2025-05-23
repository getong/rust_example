use anyhow::Result;
// BIP39 mnemonic phrase library: https://crates.io/crates/tiny-bip39
use bip39::{Language, Mnemonic, MnemonicType, Seed};

fn main() -> Result<()> {
  println!("=== tiny-bip39 BIP39 Mnemonic Phrase Example ===\n");

  // Example 1: Generate a new random 12-word mnemonic
  println!("1. Generating a new 12-word BIP39 mnemonic:");
  let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
  println!("   Mnemonic: {}", mnemonic.phrase());
  println!("   Type: 12 words");
  println!();

  // Example 2: Create mnemonic from existing phrase (for testing/validation)
  println!("2. Creating mnemonic from known test phrase:");
  let test_phrase =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
  match Mnemonic::from_phrase(test_phrase, Language::English) {
    Ok(mnemonic) => {
      println!("   ✓ Valid BIP39 mnemonic: {}", mnemonic.phrase());

      // Example 3: Generate cryptographic seed from mnemonic
      println!("\n3. Generating seed from mnemonic (for wallet derivation):");
      let seed_with_passphrase = Seed::new(&mnemonic, "optional_passphrase");
      let seed_without_passphrase = Seed::new(&mnemonic, "");

      println!("   Seed with passphrase:");
      println!(
        "     Length: {} bytes",
        seed_with_passphrase.as_bytes().len()
      );
      println!("     Hex: {}", hex::encode(seed_with_passphrase.as_bytes()));

      println!("   Seed without passphrase:");
      println!(
        "     Length: {} bytes",
        seed_without_passphrase.as_bytes().len()
      );
      println!(
        "     Hex: {}",
        hex::encode(seed_without_passphrase.as_bytes())
      );
    }
    Err(e) => println!("   ✗ Invalid mnemonic: {}", e),
  }

  // Example 4: Demonstrate different mnemonic lengths (entropy levels)
  println!("\n4. Different BIP39 mnemonic lengths and entropy:");
  let lengths = [
    (MnemonicType::Words12, "128 bits", 12),
    (MnemonicType::Words15, "160 bits", 15),
    (MnemonicType::Words18, "192 bits", 18),
    (MnemonicType::Words21, "224 bits", 21),
    (MnemonicType::Words24, "256 bits", 24),
  ];

  for (word_type, entropy, word_count) in lengths {
    let mnemonic = Mnemonic::new(word_type, Language::English);
    println!(
      "   {:2} words ({} entropy): {}",
      word_count,
      entropy,
      mnemonic.phrase()
    );
  }

  Ok(())
}
