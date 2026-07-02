use std::path::Path;

use iroh::{EndpointId, address_lookup::memory::MemoryLookup, protocol::Router};
use iroh_blobs::{
  BlobsProtocol, Hash, HashAndFormat,
  api::downloader::{DownloadOptions, DownloadProgressItem, Shuffled, SplitStrategy},
  store::fs::FsStore,
};
use n0_error::{Result, StdResultExt};
use n0_future::StreamExt;
use tokio::time::{sleep, timeout};
use tracing::{debug, warn};

use crate::{
  dht::{build_dht, discover_members, publish_member},
  endpoint::{build_endpoint, build_endpoint_with_address_lookup, endpoint_ready},
  options::{BlobGetOptions, BlobSeedOptions},
  parsing::blob_format_name,
  protocols::BLOB_PROTOCOL,
  records::{MemberRecord, member_from_endpoint_with_blobs, provider_record},
  util::display_values,
};

pub async fn run_blob_seed(options: BlobSeedOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let store = FsStore::load(&options.store_path).await.anyerr()?;
  let tag = store
    .blobs()
    .add_path(&options.file)
    .with_named_tag(blob_tag_name(&options.file))
    .await
    .anyerr()?;
  let metadata = tokio::fs::metadata(&options.file).await.anyerr()?;

  let endpoint = build_endpoint(&options.iroh, false).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let file_name = blob_file_name(&options.file);
  let member = member_from_endpoint_with_blobs(
    &endpoint,
    &options.name,
    &[BLOB_PROTOCOL],
    vec![provider_record(
      tag.hash,
      tag.format,
      file_name.clone(),
      metadata.len(),
    )],
  );
  println!("blob provider endpoint id: {}", member.endpoint_id);
  println!("blob direct addrs: {}", display_values(&member.addrs));
  if !member.relay_urls.is_empty() {
    println!("blob relay urls: {}", member.relay_urls.join(" "));
  }
  println!("blob hash: {}", tag.hash);
  println!("blob format: {}", blob_format_name(tag.format));
  println!("blob size: {} bytes", metadata.len());

  let blobs = BlobsProtocol::new(&store, None);
  let router = Router::builder(endpoint)
    .accept(iroh_blobs::ALPN, blobs)
    .spawn();

  if let Err(err) = publish_member(&dht, &options.cluster, member, 32).await {
    router.shutdown().await.std_context("shutdown router")?;
    shutdown_blob_store(&store).await;
    return Err(err);
  }

  let dht_for_publish = dht.clone();
  let cluster_for_publish = options.cluster.clone();
  let endpoint_for_publish = router.endpoint().clone();
  let name = options.name.clone();
  let hash = tag.hash;
  let format = tag.format;
  let size = metadata.len();
  let republish_every = options.republish_every;
  let republish_task = tokio::spawn(async move {
    loop {
      sleep(republish_every).await;
      let member = member_from_endpoint_with_blobs(
        &endpoint_for_publish,
        &name,
        &[BLOB_PROTOCOL],
        vec![provider_record(hash, format, file_name.clone(), size)],
      );
      if let Err(err) = publish_member(&dht_for_publish, &cluster_for_publish, member, 32).await {
        warn!("failed to republish blob provider: {err:#}");
      }
    }
  });

  println!(
    "mainline target: {} (salt: {})",
    options.cluster.target(),
    String::from_utf8_lossy(options.cluster.salt())
  );
  println!("blob seed is running. press ctrl-c to stop.");
  tokio::signal::ctrl_c().await.anyerr()?;

  republish_task.abort();
  router.shutdown().await.std_context("shutdown router")?;
  shutdown_blob_store(&store).await;
  Ok(())
}

pub async fn run_blob_get(options: BlobGetOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let address_lookup = MemoryLookup::with_provenance("mainline_kad_blobs");
  let endpoint =
    build_endpoint_with_address_lookup(&options.iroh, false, Some(address_lookup.clone())).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let discovered = match discover_members(&dht, &options.cluster, options.discover_timeout).await {
    Ok(members) => members,
    Err(err) => {
      endpoint.close().await;
      return Err(err);
    }
  };
  let providers = blob_providers(&discovered, options.hash, &address_lookup);
  println!("discovered {} blob provider(s)", providers.len());
  if providers.is_empty() {
    endpoint.close().await;
    return Err(n0_error::anyerr!(
      "no blob providers found for hash {} at target {}",
      options.hash,
      options.cluster.target()
    ));
  }

  let store = FsStore::load(&options.store_path).await.anyerr()?;
  let request = HashAndFormat::raw(options.hash);
  let downloader = store.downloader(&endpoint);
  let progress = downloader.download_with_opts(DownloadOptions::new(
    request,
    Shuffled::new(providers),
    SplitStrategy::Split,
  ));
  let mut stream = progress
    .stream()
    .await
    .map_err(|err| n0_error::anyerr!(err, "failed to start blob download"))?;
  while let Some(item) = timeout(options.request_timeout, stream.next())
    .await
    .map_err(|_| n0_error::anyerr!("blob download timed out"))?
  {
    match item {
      DownloadProgressItem::TryProvider { id, .. } => {
        println!("blob try provider: {id}");
      }
      DownloadProgressItem::ProviderFailed { id, .. } => {
        warn!("blob provider failed: {id}");
      }
      DownloadProgressItem::PartComplete { request } => {
        println!("blob part complete: {}", request.hash);
      }
      DownloadProgressItem::Progress(bytes) => {
        println!("blob downloaded: {bytes} bytes");
      }
      DownloadProgressItem::DownloadError => {
        endpoint.close().await;
        shutdown_blob_store(&store).await;
        return Err(n0_error::anyerr!("blob download failed"));
      }
      DownloadProgressItem::Error(err) => {
        endpoint.close().await;
        shutdown_blob_store(&store).await;
        return Err(err);
      }
    }
  }

  let bitfield = store
    .blobs()
    .observe(options.hash)
    .await_completion()
    .await
    .anyerr()?;
  if !bitfield.is_complete() {
    endpoint.close().await;
    shutdown_blob_store(&store).await;
    return Err(n0_error::anyerr!(
      "blob {} is not complete after download",
      options.hash
    ));
  }

  let exported = store
    .blobs()
    .export(options.hash, &options.output)
    .await
    .anyerr()?;
  println!(
    "blob export complete: {} bytes -> {}",
    exported,
    options.output.display()
  );

  endpoint.close().await;
  shutdown_blob_store(&store).await;
  Ok(())
}

fn blob_providers(
  members: &[MemberRecord],
  hash: Hash,
  address_lookup: &MemoryLookup,
) -> Vec<EndpointId> {
  let mut providers = Vec::new();

  for member in members {
    if !member.supports_blob() {
      continue;
    }

    let provides_hash = member.blobs.iter().any(|record| {
      record
        .hash_and_format()
        .map(|hash_and_format| hash_and_format.hash == hash)
        .unwrap_or(false)
    });
    if !provides_hash {
      continue;
    }

    match member.endpoint_addr() {
      Ok(addr) => {
        println!("blob provider: {} ({})", member.name, addr.id);
        address_lookup.add_endpoint_info(addr.clone());
        providers.push(addr.id);
      }
      Err(err) => {
        warn!("skipping invalid blob provider {}: {err:#}", member.name);
      }
    }
  }

  providers
}

fn blob_tag_name(path: &Path) -> String {
  format!(
    "mainline-kad:{}",
    path
      .file_name()
      .and_then(|name| name.to_str())
      .unwrap_or("blob")
  )
}

fn blob_file_name(path: &Path) -> String {
  path
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("blob")
    .to_string()
}

async fn shutdown_blob_store(store: &FsStore) {
  if let Err(err) = store.shutdown().await {
    debug!("blob store shutdown returned after close: {err:#}");
  }
}
