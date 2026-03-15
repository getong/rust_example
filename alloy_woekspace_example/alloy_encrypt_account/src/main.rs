use std::{env, fs, path::Path};

use aes_gcm::{
  Aes256Gcm, Nonce,
  aead::{Aead, KeyInit},
};
use alloy::{network::EthereumWallet, signers::local::PrivateKeySigner};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Context, Result, bail, ensure, eyre};
use pbkdf2::pbkdf2_hmac;
use rand::{TryRng, rngs::SysRng};
use sha2::Sha256;

const ENV_PATH: &str = ".env";
const ENCRYPTED_PRIVATE_KEY_KEY: &str = "ENCRYPTED_PRIVATE_KEY";
const LEGACY_PRIVATE_KEY_KEY: &str = "PRIVATE_KEY";
const PASSWORD_ENV_KEY: &str = "PRIVATE_KEY_PASSWORD";
const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const ENCRYPTED_VALUE_PREFIX: &str = "v1:";

fn main() -> Result<()> {
  dotenv::dotenv().ok();

  let mut args = env::args().skip(1);
  match args.next().as_deref() {
    Some("encrypt") => encrypt_private_key_into_env(),
    Some("show-address") | None => load_wallet_from_env(),
    Some(other) => bail!("Unknown command: {other}. Supported commands: `encrypt`, `show-address`"),
  }
}

fn encrypt_private_key_into_env() -> Result<()> {
  let private_key = read_private_key_for_encryption()?;
  validate_private_key(&private_key)?;

  let password = read_password(
    "Enter encryption password: ",
    "Set PRIVATE_KEY_PASSWORD in your shell or enter it interactively.",
  )?;
  let password_confirm = read_password(
    "Confirm encryption password: ",
    "Set PRIVATE_KEY_PASSWORD in your shell or enter it interactively.",
  )?;
  ensure!(password == password_confirm, "Passwords do not match");

  let encrypted_private_key = encrypt_secret(&private_key, &password)?;
  upsert_env_value(ENV_PATH, ENCRYPTED_PRIVATE_KEY_KEY, &encrypted_private_key)?;
  remove_env_key(ENV_PATH, LEGACY_PRIVATE_KEY_KEY)?;

  println!("Encrypted private key written to {}", ENV_PATH);
  println!(
    "The raw PRIVATE_KEY entry has been removed from {}. Keep the password outside .env.",
    ENV_PATH
  );

  Ok(())
}

fn read_private_key_for_encryption() -> Result<String> {
  if let Ok(private_key) = env::var(LEGACY_PRIVATE_KEY_KEY) {
    ensure!(
      !private_key.is_empty(),
      "PRIVATE_KEY is set but empty. Provide a valid 0x-prefixed private key."
    );
    return Ok(private_key);
  }

  let private_key = rpassword::prompt_password("Enter raw private key to encrypt: ")
    .wrap_err("Failed to read private key from terminal")?;
  ensure!(
    !private_key.is_empty(),
    "Private key cannot be empty. Enter a valid 0x-prefixed private key."
  );

  Ok(private_key)
}

fn load_wallet_from_env() -> Result<()> {
  let encrypted_private_key = env::var(ENCRYPTED_PRIVATE_KEY_KEY).wrap_err(
    "ENCRYPTED_PRIVATE_KEY is not set. Run `cargo run -p alloy_encrypt_account -- encrypt` first.",
  )?;

  let password = read_password(
    "Enter decryption password: ",
    "Set PRIVATE_KEY_PASSWORD in your shell or enter it interactively.",
  )?;
  let private_key = decrypt_secret(&encrypted_private_key, &password)?;
  let signer: PrivateKeySigner = private_key
    .parse()
    .wrap_err("Decrypted private key is invalid. Check the password and ciphertext.")?;
  let _wallet = EthereumWallet::from(signer.clone());

  println!("Encrypted private key loaded from .env");
  println!("Signer address: {}", signer.address());
  println!("EthereumWallet initialized successfully");

  Ok(())
}

fn validate_private_key(private_key: &str) -> Result<()> {
  let _: PrivateKeySigner = private_key
    .parse()
    .wrap_err("PRIVATE_KEY format is invalid. alloy expects a 0x-prefixed 32-byte hex string")?;
  Ok(())
}

fn read_password(prompt: &str, missing_hint: &str) -> Result<String> {
  if let Ok(password) = env::var(PASSWORD_ENV_KEY) {
    ensure!(
      !password.is_empty(),
      "{PASSWORD_ENV_KEY} is set but empty. {missing_hint}"
    );
    return Ok(password);
  }

  let password =
    rpassword::prompt_password(prompt).wrap_err("Failed to read password from terminal")?;
  ensure!(
    !password.is_empty(),
    "Password cannot be empty. {missing_hint}"
  );
  Ok(password)
}

fn encrypt_secret(secret: &str, password: &str) -> Result<String> {
  let mut salt = [0_u8; SALT_LEN];
  let mut nonce_bytes = [0_u8; NONCE_LEN];
  let mut rng = SysRng;
  rng
    .try_fill_bytes(&mut salt)
    .map_err(|err| eyre!("Failed to generate random salt: {err}"))?;
  rng
    .try_fill_bytes(&mut nonce_bytes)
    .map_err(|err| eyre!("Failed to generate random nonce: {err}"))?;

  let key = derive_key(password.as_bytes(), &salt);
  let cipher = Aes256Gcm::new_from_slice(&key).wrap_err("Failed to initialize AES-256-GCM")?;
  let nonce = Nonce::from_slice(&nonce_bytes);
  let ciphertext = cipher
    .encrypt(nonce, secret.as_bytes())
    .map_err(|_| eyre!("Failed to encrypt private key"))?;

  let mut payload = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
  payload.extend_from_slice(&salt);
  payload.extend_from_slice(&nonce_bytes);
  payload.extend_from_slice(&ciphertext);

  Ok(format!(
    "{ENCRYPTED_VALUE_PREFIX}{}",
    STANDARD.encode(payload)
  ))
}

fn decrypt_secret(encrypted_secret: &str, password: &str) -> Result<String> {
  let payload = encrypted_secret
    .strip_prefix(ENCRYPTED_VALUE_PREFIX)
    .ok_or_else(|| eyre!("Encrypted value format is invalid. Missing version prefix."))?;
  let decoded = STANDARD
    .decode(payload)
    .wrap_err("Encrypted value is not valid base64")?;
  ensure!(
    decoded.len() > SALT_LEN + NONCE_LEN,
    "Encrypted value is too short to contain salt, nonce and ciphertext"
  );

  let salt = &decoded[.. SALT_LEN];
  let nonce_bytes = &decoded[SALT_LEN .. SALT_LEN + NONCE_LEN];
  let ciphertext = &decoded[SALT_LEN + NONCE_LEN ..];

  let key = derive_key(password.as_bytes(), salt);
  let cipher = Aes256Gcm::new_from_slice(&key).wrap_err("Failed to initialize AES-256-GCM")?;
  let nonce = Nonce::from_slice(nonce_bytes);
  let plaintext = cipher
    .decrypt(nonce, ciphertext)
    .map_err(|_| eyre!("Failed to decrypt private key. Password may be wrong."))?;

  String::from_utf8(plaintext).wrap_err("Decrypted private key is not valid UTF-8")
}

fn derive_key(password: &[u8], salt: &[u8]) -> [u8; KEY_LEN] {
  let mut key = [0_u8; KEY_LEN];
  pbkdf2_hmac::<Sha256>(password, salt, PBKDF2_ITERATIONS, &mut key);
  key
}

fn upsert_env_value(path: &str, key: &str, value: &str) -> Result<()> {
  let path = Path::new(path);
  let mut lines = if path.exists() {
    fs::read_to_string(path)
      .wrap_err_with(|| format!("Failed to read {}", path.display()))?
      .lines()
      .map(str::to_owned)
      .collect::<Vec<_>>()
  } else {
    Vec::new()
  };

  let new_line = format!("{key}={value}");
  let mut replaced = false;
  for line in &mut lines {
    if line.starts_with(&format!("{key}=")) {
      *line = new_line.clone();
      replaced = true;
    }
  }
  if !replaced {
    lines.push(new_line);
  }

  let content = if lines.is_empty() {
    String::new()
  } else {
    format!("{}\n", lines.join("\n"))
  };
  fs::write(path, content).wrap_err_with(|| format!("Failed to write {}", path.display()))
}

fn remove_env_key(path: &str, key: &str) -> Result<()> {
  let path = Path::new(path);
  if !path.exists() {
    return Ok(());
  }

  let content =
    fs::read_to_string(path).wrap_err_with(|| format!("Failed to read {}", path.display()))?;
  let filtered = content
    .lines()
    .filter(|line| !line.starts_with(&format!("{key}=")))
    .collect::<Vec<_>>()
    .join("\n");
  let final_content = if filtered.is_empty() {
    String::new()
  } else {
    format!("{filtered}\n")
  };

  fs::write(path, final_content).wrap_err_with(|| format!("Failed to write {}", path.display()))
}
