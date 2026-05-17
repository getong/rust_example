use std::{env, path::Path};

use anyhow::{Context, Result};
use fabric_sdk::{gateway::client::ClientBuilder, identity::IdentityBuilder};

/// Demonstrates configuring the Fabric client using environment
/// variables (e.g. from a .env file).
///
/// Expected environment variables:
///   FABRIC_CERT_PATH        - path to the signcert PEM file
///   FABRIC_KEY_PATH         - path to the private key PEM file
///   FABRIC_TLS_CERT_PATH    - path to the TLS CA cert PEM file
///   FABRIC_MSP_ID           - MSP identifier (default: Org1MSP)
///   FABRIC_PEER_ENDPOINT    - peer gRPC endpoint (default: localhost:7051)
///
/// Usage:
///   cargo run --example 06_env_config
#[tokio::main]
async fn main() -> Result<()> {
  dotenv::dotenv().ok();

  // --- Load configuration from environment ---
  let cert_path = env::var("FABRIC_CERT_PATH").context("FABRIC_CERT_PATH not set")?;
  let key_path = env::var("FABRIC_KEY_PATH").context("FABRIC_KEY_PATH not set")?;
  let tls_cert_path = env::var("FABRIC_TLS_CERT_PATH").context("FABRIC_TLS_CERT_PATH not set")?;

  let msp_id = env::var("FABRIC_MSP_ID").unwrap_or_else(|_| "Org1MSP".into());
  let peer_endpoint = env::var("FABRIC_PEER_ENDPOINT").unwrap_or_else(|_| "localhost:7051".into());

  // --- Read PEM files ---
  let cert_pem = std::fs::read(Path::new(&cert_path)).context("Failed to read certificate file")?;
  let key_pem =
    std::fs::read_to_string(Path::new(&key_path)).context("Failed to read private key file")?;
  let tls_ca_pem =
    std::fs::read(Path::new(&tls_cert_path)).context("Failed to read TLS CA certificate file")?;

  // --- Build identity ---
  let identity = IdentityBuilder::from_pem(&cert_pem)?
    .with_msp(&msp_id)?
    .with_private_key(key_pem)?
    .build()?;

  // --- Build and connect client ---
  let mut client = ClientBuilder::new()
    .with_identity(identity)?
    .with_tls(tls_ca_pem)?
    .with_scheme("https")?
    .with_authority(&peer_endpoint)?
    .build()?;

  client.connect().await?;
  println!("Connected to Fabric peer at {peer_endpoint}");

  // --- Query example ---
  // (Uncomment when your network is ready)
  // let signed_proposal = client
  //     .get_chaincode_call_builder()
  //     .with_channel_name("mychannel")?
  //     .with_chaincode_id("basic")?
  //     .with_function_name("ReadAsset")?
  //     .with_function_args(["asset1"])?
  //     .build()?;
  //
  // let result = client
  //     .evaluate(signed_proposal, String::new(), "mychannel".into())
  //     .await?;
  // println!("Query result: {}", String::from_utf8_lossy(&result));

  Ok(())
}
