use anyhow::{Context, Result};
use fabric_sdk::{gateway::client::ClientBuilder, identity::IdentityBuilder};
use std::path::Path;

/// Demonstrates subscribing to chaincode events.
///
/// Prerequisites:
///   - Running Hyperledger Fabric test network
///   - Set env vars: FABRIC_CERT_PATH, FABRIC_KEY_PATH, FABRIC_TLS_CERT_PATH
///
/// Usage:
///   export FABRIC_CERT_PATH=.../signcerts/User1@org1.example.com-cert.pem
///   export FABRIC_KEY_PATH=.../keystore/priv_sk
///   export FABRIC_TLS_CERT_PATH=.../tlsca/tlsca.org1.example.com-cert.pem
///   cargo run --example 04_chaincode_events
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

    let events_request = client.get_chaincode_events_request_builder().build()?;

    let mut stream = client.chaincode_events(events_request).await?;

    println!("Listening for chaincode events...");

    use tokio_stream::StreamExt;
    while let Some(event) = stream.next().await {
        match event {
            Ok(evt) => println!("Event: block={:?}", evt),
            Err(e) => eprintln!("Error: {e}"),
        }
    }

    Ok(())
}
