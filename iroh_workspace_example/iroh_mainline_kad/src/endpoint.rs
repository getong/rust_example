use std::time::Duration;

use iroh::{
  Endpoint, RelayMode, SecretKey, address_lookup::memory::MemoryLookup, endpoint::presets,
};
use n0_error::{Result, StdResultExt};
use tokio::time::timeout;

use crate::{options::IrohOptions, protocols::CLUSTER_ALPN};

pub(crate) async fn build_endpoint(options: &IrohOptions, accept: bool) -> Result<Endpoint> {
  build_endpoint_with_address_lookup(options, accept, None).await
}

pub(crate) async fn build_endpoint_with_address_lookup(
  options: &IrohOptions,
  accept: bool,
  address_lookup: Option<MemoryLookup>,
) -> Result<Endpoint> {
  let mut builder = Endpoint::builder(presets::N0)
    .secret_key(SecretKey::generate())
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
