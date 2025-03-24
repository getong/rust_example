use std::{fs, path::Path};

use rcgen::{CertifiedKey, generate_simple_self_signed};

const CERTS_DIR: &str = "/tmp/quinn-chat-certs";

fn main() {
  let subject_alt_names = vec!["localhost".to_string()];

  let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names).unwrap();

  if !Path::new(CERTS_DIR).exists() {
    fs::create_dir_all(CERTS_DIR).expect("Failed to create 'certs' directory");
  }

  let _ = fs::write(format!("{}/cert.pem", CERTS_DIR), cert.pem());
  let _ = fs::write(format!("{}/cert.key", CERTS_DIR), key_pair.serialize_pem());
}

// copy from https://github.com/Matheus-git/quic-chat
