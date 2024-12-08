use identity_iota::{
  core::ToJson,
  iota::{IotaClientExt, IotaDocument, IotaIdentityClientExt, NetworkName},
  storage::{JwkDocumentExt, JwkMemStore, KeyIdMemstore, Storage},
  verification::{MethodScope, jws::JwsAlgorithm},
};
use iota_sdk::{
  client::{
    Client,
    api::GetAddressesOptions,
    secret::{SecretManager, stronghold::StrongholdSecretManager},
  },
  crypto::keys::bip39,
  types::block::{
    address::Bech32Address,
    output::{AliasOutput, dto::AliasOutputDto},
  },
};
use tokio::io::AsyncReadExt;

// The endpoint of the IOTA node to use.
static API_ENDPOINT: &str = "http://localhost";

/// Demonstrates how to create a DID Document and publish it in a new Alias Output.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Create a new client to interact with the IOTA ledger.
  let client: Client = Client::builder()
    .with_primary_node(API_ENDPOINT, None)?
    .finish()
    .await?;

  // Create a new Stronghold.
  let stronghold = StrongholdSecretManager::builder()
    .password("secure_password".to_owned())
    .build("./example-strong.hodl")?;

  // Generate a mnemonic and store it in the Stronghold.
  let random: [u8; 32] = rand::random();
  let mnemonic = bip39::wordlist::encode(random.as_ref(), &bip39::wordlist::ENGLISH)
    .map_err(|err| anyhow::anyhow!("{err:?}"))?;
  stronghold.store_mnemonic(mnemonic).await?;

  // Create a new secret manager backed by the Stronghold.
  let secret_manager: SecretManager = SecretManager::Stronghold(stronghold);

  // Get the Bech32 human-readable part (HRP) of the network.
  let network_name: NetworkName = client.network_name().await?;

  // Get an address from the secret manager.
  let address: Bech32Address = secret_manager
    .generate_ed25519_addresses(
      GetAddressesOptions::default()
        .with_range(0 .. 1)
        .with_bech32_hrp((&network_name).try_into()?),
    )
    .await?[0];

  println!("Your wallet address is: {}", address);
  println!(
    "Please request funds from http://localhost/faucet/, wait for a couple of seconds and then \
     press Enter."
  );
  tokio::io::stdin().read_u8().await?;

  // Create a new DID document with a placeholder DID.
  // The DID will be derived from the Alias Id of the Alias Output after publishing.
  let mut document: IotaDocument = IotaDocument::new(&network_name);

  // Insert a new Ed25519 verification method in the DID document.
  let storage: Storage<JwkMemStore, KeyIdMemstore> =
    Storage::new(JwkMemStore::new(), KeyIdMemstore::new());
  document
    .generate_method(
      &storage,
      JwkMemStore::ED25519_KEY_TYPE,
      JwsAlgorithm::EdDSA,
      None,
      MethodScope::VerificationMethod,
    )
    .await?;

  // Construct an Alias Output containing the DID document, with the wallet address
  // set as both the state controller and governor.
  let alias_output: AliasOutput = client
    .new_did_output(address.into(), document, None)
    .await?;
  println!(
    "Alias Output: {}",
    AliasOutputDto::from(&alias_output).to_json_pretty()?
  );

  // Publish the Alias Output and get the published DID document.
  let document: IotaDocument = client
    .publish_did_output(&secret_manager, alias_output)
    .await?;
  println!("Published DID document: {:#}", document);

  Ok(())
}
