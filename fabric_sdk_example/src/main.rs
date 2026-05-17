use anyhow::{Context, Result};
use fabric_sdk::{gateway::client::ClientBuilder, identity::IdentityBuilder};
use std::path::Path;

/// Example 1: Create identity from PEM certificate bytes
fn build_identity(
    cert_pem: &[u8],
    msp_id: &str,
    private_key_pem: Option<String>,
) -> Result<fabric_sdk::identity::Identity> {
    let mut builder = IdentityBuilder::from_pem(cert_pem)?.with_msp(msp_id)?;
    if let Some(key) = private_key_pem {
        builder = builder.with_private_key(key)?;
    }
    Ok(builder.build()?)
}

/// Example 2: Build and connect a Fabric Gateway client
async fn create_and_connect_client(
    identity: fabric_sdk::identity::Identity,
    tls_cert: Option<Vec<u8>>,
    authority: &str,
) -> Result<fabric_sdk::gateway::client::Client> {
    let mut builder = ClientBuilder::new()
        .with_identity(identity)?
        .with_scheme("https")?
        .with_authority(authority)?;

    if let Some(tls) = tls_cert {
        builder = builder.with_tls(tls)?;
    }

    let mut client = builder.build()?;
    client.connect().await?;
    Ok(client)
}

/// Example 3: Query chaincode (read-only, evaluate)
async fn query_chaincode(
    client: &fabric_sdk::gateway::client::Client,
    channel_name: &str,
    chaincode_id: &str,
    function_name: &str,
    args: Vec<&str>,
) -> Result<Vec<u8>> {
    let signed_proposal = client
        .get_chaincode_call_builder()
        .with_channel_name(channel_name)?
        .with_chaincode_id(chaincode_id)?
        .with_function_name(function_name)?
        .with_function_args(args)?
        .build()?;

    let result = client
        .evaluate(signed_proposal, String::new(), channel_name.to_string())
        .await?;
    Ok(result)
}

/// Example 4: Submit chaincode transaction (endorse + commit)
async fn submit_chaincode_transaction(
    client: &fabric_sdk::gateway::client::Client,
    channel_name: &str,
    chaincode_id: &str,
    function_name: &str,
    args: Vec<&str>,
) -> Result<Vec<u8>> {
    let signed_proposal = client
        .get_chaincode_call_builder()
        .with_channel_name(channel_name)?
        .with_chaincode_id(chaincode_id)?
        .with_function_name(function_name)?
        .with_function_args(args)?
        .build()?;

    let response = client.process_proposal(signed_proposal).await?;
    let result = response
        .response
        .as_ref()
        .map(|r| r.payload.clone())
        .unwrap_or_default();
    Ok(result)
}

/// Example 5: Check commit status of a transaction (kept for reference)
#[allow(dead_code)]
async fn check_commit_status(
    client: &fabric_sdk::gateway::client::Client,
    transaction_id: String,
    channel_id: String,
) -> Result<()> {
    let status = client.commit_status(transaction_id, channel_id).await?;
    println!("Commit status: {:?}", status);
    Ok(())
}

/// Example 6: Use ChaincodeCallBuilder directly for more control
async fn chaincode_call_builder_example(
    client: &fabric_sdk::gateway::client::Client,
) -> Result<()> {
    let mut builder = client.get_chaincode_call_builder();
    builder
        .with_channel_name("mychannel")?
        .with_chaincode_id("basic")?
        .with_function_name("ReadAsset")?
        .with_function_args(["asset1"])?;

    let signed_proposal = builder.build()?;

    let result = client
        .evaluate(signed_proposal, String::new(), "mychannel".into())
        .await?;

    println!("Query result: {}", String::from_utf8_lossy(&result));
    Ok(())
}

/// Example 7: Lifecycle client - install/approve/commit chaincode
async fn lifecycle_example(client: &fabric_sdk::gateway::client::Client) -> Result<()> {
    let _lifecycle = client.get_lifecycle_client();
    println!("Lifecycle client created");
    Ok(())
}

fn load_env_or_exit() -> Result<(
    Vec<u8>,  // cert_pem
    Vec<u8>,  // tls_ca_pem
    String,   // private_key_pem
    String,   // msp_id
    String,   // peer_endpoint
)> {
    dotenv::dotenv().ok();

    let cert_path = std::env::var("FABRIC_CERT_PATH")
        .context("FABRIC_CERT_PATH not set — see instructions below")?;
    let key_path = std::env::var("FABRIC_KEY_PATH")
        .context("FABRIC_KEY_PATH not set")?;
    let tls_path = std::env::var("FABRIC_TLS_CERT_PATH")
        .context("FABRIC_TLS_CERT_PATH not set")?;

    let msp_id = std::env::var("FABRIC_MSP_ID").unwrap_or_else(|_| "Org1MSP".into());
    let endpoint = std::env::var("FABRIC_PEER_ENDPOINT").unwrap_or_else(|_| "localhost:7051".into());

    let cert_pem = std::fs::read(Path::new(&cert_path))
        .context("Failed to read FABRIC_CERT_PATH")?;
    let key_pem = std::fs::read_to_string(Path::new(&key_path))
        .context("Failed to read FABRIC_KEY_PATH")?;
    let tls_ca_pem = std::fs::read(Path::new(&tls_path))
        .context("Failed to read FABRIC_TLS_CERT_PATH")?;

    Ok((cert_pem, tls_ca_pem, key_pem, msp_id, endpoint))
}

fn print_instructions() {
    println!("=== Fabric SDK Rust Examples ===\n");
    println!("NOTE: No Fabric network credentials found.\n");
    println!("To run this example, set the following environment variables\n");
    println!("  export FABRIC_CERT_PATH=.../signcerts/User1@org1.example.com-cert.pem");
    println!("  export FABRIC_KEY_PATH=.../keystore/priv_sk");
    println!("  export FABRIC_TLS_CERT_PATH=.../tlsca/tlsca.org1.example.com-cert.pem");
    println!("  export FABRIC_MSP_ID=Org1MSP                     (optional)");
    println!("  export FABRIC_PEER_ENDPOINT=localhost:7051        (optional)\n");
    println!("Or create a .env file with those values.\n");
    println!("See examples/ directory for focused standalone examples:\n");
    println!("  cargo run --example 01_connect_and_query");
    println!("  cargo run --example 02_submit_transaction");
    println!("  cargo run --example 03_lifecycle_chaincode");
    println!("  cargo run --example 04_chaincode_events");
    println!("  cargo run --example 05_discovery");
    println!("  cargo run --example 06_env_config\n");
    println!("A running Hyperledger Fabric test network with the basic-asset");
    println!("chaincode on channel \"mychannel\" is required.\n");
}

#[tokio::main]
async fn main() -> Result<()> {
    let (cert_pem, tls_ca_pem, key_pem, msp_id, endpoint) = match load_env_or_exit() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {e:#}\n");
            print_instructions();
            std::process::exit(0);
        }
    };

    println!("=== Fabric SDK Rust Examples ===\n");

    // Example 1: Build identity
    println!("1. Building identity...");
    let identity = build_identity(&cert_pem, &msp_id, Some(key_pem))?;
    println!("   Identity created with MSP: {msp_id}");

    // Example 2: Build and connect client
    println!("\n2. Creating and connecting client...");
    let client = create_and_connect_client(identity, Some(tls_ca_pem), &endpoint).await?;
    println!("   Client connected to {endpoint}");

    // Example 3: Query chaincode
    println!("\n3. Querying chaincode (evaluate)...");
    match query_chaincode(&client, "mychannel", "basic", "ReadAsset", vec!["asset1"]).await {
        Ok(result) => println!("   Query result: {}", String::from_utf8_lossy(&result)),
        Err(e) => println!("   Query failed (expected without network): {e}"),
    }

    // Example 4: Submit transaction
    println!("\n4. Submitting chaincode transaction...");
    match submit_chaincode_transaction(
        &client,
        "mychannel",
        "basic",
        "CreateAsset",
        vec!["asset1", "blue", "10", "Alice", "100"],
    )
    .await
    {
        Ok(result) => println!("   Submit result: {}", String::from_utf8_lossy(&result)),
        Err(e) => println!("   Submit failed (expected without network): {e}"),
    }

    // Example 5: ChaincodeCallBuilder with more control
    println!("\n5. Using ChaincodeCallBuilder directly...");
    chaincode_call_builder_example(&client).await.ok();

    // Example 6: Lifecycle client
    println!("\n6. Getting lifecycle client...");
    lifecycle_example(&client).await?;

    println!("\n=== All examples completed ===");
    Ok(())
}
