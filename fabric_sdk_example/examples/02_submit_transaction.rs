use anyhow::{Context, Result};
use fabric_sdk::{gateway::client::ClientBuilder, identity::IdentityBuilder};
use std::path::Path;

/// Demonstrates submitting a chaincode transaction (write operation)
/// that creates an asset on the ledger.
///
/// Prerequisites:
///   - Running Hyperledger Fabric test network with basic-asset chaincode
///     on channel "mychannel"
///   - Set env vars: FABRIC_CERT_PATH, FABRIC_KEY_PATH, FABRIC_TLS_CERT_PATH
///
/// Usage:
///   export FABRIC_CERT_PATH=.../signcerts/User1@org1.example.com-cert.pem
///   export FABRIC_KEY_PATH=.../keystore/priv_sk
///   export FABRIC_TLS_CERT_PATH=.../tlsca/tlsca.org1.example.com-cert.pem
///   cargo run --example 02_submit_transaction
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let cert_path = std::env::var("FABRIC_CERT_PATH")
        .context("FABRIC_CERT_PATH not set")?;
    let key_path = std::env::var("FABRIC_KEY_PATH")
        .context("FABRIC_KEY_PATH not set")?;
    let tls_path = std::env::var("FABRIC_TLS_CERT_PATH")
        .context("FABRIC_TLS_CERT_PATH not set")?;

    let cert_pem = std::fs::read(Path::new(&cert_path))?;
    let key_pem = std::fs::read_to_string(Path::new(&key_path))?;
    let tls_ca_pem = std::fs::read(Path::new(&tls_path))?;

    let identity = IdentityBuilder::from_pem(&cert_pem)?
        .with_msp("Org1MSP")?
        .with_private_key(key_pem)?
        .build()?;

    let mut client = ClientBuilder::new()
        .with_identity(identity)?
        .with_tls(tls_ca_pem)?
        .with_scheme("https")?
        .with_authority("localhost:7051")?
        .build()?;

    client.connect().await?;
    println!("Connected to Fabric peer");

    let signed_proposal = client
        .get_chaincode_call_builder()
        .with_channel_name("mychannel")?
        .with_chaincode_id("basic")?
        .with_function_name("CreateAsset")?
        .with_function_args(["asset99", "red", "5", "Bob", "200"])?
        .build()?;

    let response = client.process_proposal(signed_proposal).await?;
    println!("Proposal endorsed successfully");

    let payload = response
        .response
        .as_ref()
        .map(|r| String::from_utf8_lossy(&r.payload).to_string())
        .unwrap_or_default();
    println!("Response: {payload}");

    Ok(())
}
