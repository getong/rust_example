use std::process::ExitCode;

#[cfg(feature = "system-rng")]
use clatter_example::run_hybrid_exchange;

fn main() -> ExitCode {
  #[cfg(feature = "system-rng")]
  {
    match run_hybrid_exchange() {
      Ok(outcome) => {
        println!("Clatter hybrid Noise + Post-Quantum demo");
        println!("pattern: Noise_hybridNN_X25519+MLKEM512_AESGCM_SHA512");
        println!(
          "handshake messages: {} bytes ->, {} bytes <-",
          outcome.first_handshake_message_len, outcome.second_handshake_message_len
        );
        println!(
          "transport frames: {} bytes Alice->Bob, {} bytes Bob->Alice",
          outcome.alice_to_bob_ciphertext_len, outcome.bob_to_alice_ciphertext_len
        );
        print_plaintext("Bob decrypted", outcome.bob_plaintext());
        print_plaintext("Alice decrypted", outcome.alice_plaintext());
        ExitCode::SUCCESS
      }
      Err(error) => {
        eprintln!("demo failed: {error}");
        ExitCode::FAILURE
      }
    }
  }

  #[cfg(not(feature = "system-rng"))]
  {
    println!("enable the `system-rng` feature to run the host demo");
    ExitCode::SUCCESS
  }
}

fn print_plaintext(label: &str, bytes: &[u8]) {
  match std::str::from_utf8(bytes) {
    Ok(text) => println!("{label}: {text}"),
    Err(_) => println!("{label}: {bytes:?}"),
  }
}
