fn main() -> Result<(), Box<dyn std::error::Error>> {
  serve_pem::ensure_crypto_files().map_err(std::io::Error::other)?;

  println!("Wrote RSA key material:");
  println!("  - private_key.pk8");
  println!("  - public_key.der");

  Ok(())
}
