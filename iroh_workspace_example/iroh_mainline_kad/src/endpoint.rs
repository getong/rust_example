use std::{path::Path, time::Duration};

use iroh::{
  Endpoint, RelayMode, SecretKey, address_lookup::memory::MemoryLookup, endpoint::presets,
};
use n0_error::{Result, StdResultExt};
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use crate::{
  options::IrohOptions, parsing::parse_iroh_secret_key, protocols::CLUSTER_ALPN, util::hex_encode,
};

pub(crate) async fn build_endpoint(options: &IrohOptions, accept: bool) -> Result<Endpoint> {
  build_endpoint_with_address_lookup(options, accept, None).await
}

pub(crate) async fn build_endpoint_with_address_lookup(
  options: &IrohOptions,
  accept: bool,
  address_lookup: Option<MemoryLookup>,
) -> Result<Endpoint> {
  let secret_key = match options.secret_path.as_deref() {
    Some(path) => load_or_generate_secret_key(path).await?,
    None => SecretKey::generate(),
  };

  let mut builder = Endpoint::builder(presets::N0)
    .secret_key(secret_key)
    .relay_mode(if options.relay {
      RelayMode::Default
    } else {
      RelayMode::Disabled
    });

  if let Some(address_lookup) = address_lookup {
    builder = builder.address_lookup(address_lookup);
  }

  if accept {
    builder = builder.alpns(vec![CLUSTER_ALPN.to_vec()]);
  }

  builder = builder
    .clear_ip_transports()
    .bind_addr(options.bind)
    .anyerr()?;
  builder.bind().await.anyerr()
}

async fn load_or_generate_secret_key(path: &Path) -> Result<SecretKey> {
  match tokio::fs::read_to_string(path).await {
    Ok(secret) => Ok(SecretKey::from_bytes(&parse_iroh_secret_key(
      secret.trim(),
    )?)),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      let secret_key = SecretKey::generate();
      persist_secret_key(path, &secret_key).await?;
      Ok(secret_key)
    }
    Err(err) => Err(err).anyerr(),
  }
}

async fn persist_secret_key(path: &Path, secret_key: &SecretKey) -> Result<()> {
  if let Some(parent) = path.parent()
    && !parent.as_os_str().is_empty()
  {
    tokio::fs::create_dir_all(parent).await.anyerr()?;
  }

  let encoded = format!("{}\n", hex_encode(&secret_key.to_bytes()));
  let mut options = tokio::fs::OpenOptions::new();
  options.write(true).create_new(true);
  set_secret_file_open_options(&mut options);

  let mut file = options.open(path).await.anyerr()?;
  file.write_all(encoded.as_bytes()).await.anyerr()?;
  file.sync_all().await.anyerr()?;
  Ok(())
}

#[cfg(unix)]
fn set_secret_file_open_options(options: &mut tokio::fs::OpenOptions) {
  options.mode(0o600);
}

#[cfg(not(unix))]
fn set_secret_file_open_options(_options: &mut tokio::fs::OpenOptions) {}

pub(crate) async fn endpoint_ready(
  endpoint: &Endpoint,
  relay: bool,
  wait_online: Duration,
) -> Result<()> {
  if relay && !wait_online.is_zero() {
    timeout(wait_online, endpoint.online()).await.anyerr()?;
  }
  Ok(())
}
