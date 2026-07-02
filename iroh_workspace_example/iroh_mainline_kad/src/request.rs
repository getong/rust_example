use std::time::Duration;

use iroh::{Endpoint, EndpointId, endpoint::ConnectionError};
use n0_error::{Result, StdResultExt};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

use crate::{
  dht::{build_dht, discover_members, publish_member},
  endpoint::{build_endpoint, endpoint_ready},
  options::{ClientOptions, ServerOptions},
  protocols::{CLUSTER_ALPN, REQUEST_PROTOCOL},
  records::{MemberRecord, member_from_endpoint},
};

pub async fn run_server(options: ServerOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let endpoint = build_endpoint(&options.iroh, true).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let member = member_from_endpoint(&endpoint, &options.name, &[REQUEST_PROTOCOL]);
  println!("iroh endpoint id: {}", member.endpoint_id);
  println!("iroh direct addrs: {}", member.addrs.join(" "));
  if !member.relay_urls.is_empty() {
    println!("iroh relay urls: {}", member.relay_urls.join(" "));
  }

  if let Err(err) = publish_member(&dht, &options.cluster, member, 16).await {
    endpoint.close().await;
    return Err(err);
  }

  let endpoint_for_accept = endpoint.clone();
  let accept_task = tokio::spawn(async move {
    if let Err(err) = accept_loop(endpoint_for_accept).await {
      eprintln!("accept loop stopped: {err:#}");
    }
  });

  let dht_for_publish = dht.clone();
  let cluster_for_publish = options.cluster.clone();
  let endpoint_for_publish = endpoint.clone();
  let name = options.name.clone();
  let republish_every = options.republish_every;
  let republish_task = tokio::spawn(async move {
    loop {
      sleep(republish_every).await;
      let member = member_from_endpoint(&endpoint_for_publish, &name, &[REQUEST_PROTOCOL]);
      if let Err(err) = publish_member(&dht_for_publish, &cluster_for_publish, member, 16).await {
        warn!("failed to republish cluster member: {err:#}");
      }
    }
  });

  println!(
    "mainline target: {} (salt: {})",
    options.cluster.target(),
    String::from_utf8_lossy(options.cluster.salt())
  );
  println!("server is running. press ctrl-c to stop.");
  tokio::signal::ctrl_c().await.anyerr()?;

  accept_task.abort();
  republish_task.abort();
  endpoint.close().await;
  Ok(())
}

pub async fn run_client(options: ClientOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let endpoint = build_endpoint(&options.iroh, false).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let members = match discover_members(&dht, &options.cluster, options.discover_timeout).await {
    Ok(members) => members,
    Err(err) => {
      endpoint.close().await;
      return Err(err);
    }
  };
  let members = members
    .into_iter()
    .filter(MemberRecord::supports_request)
    .collect::<Vec<_>>();
  println!("discovered {} request member(s)", members.len());

  let mut last_error = None;
  for member in members {
    println!("dialing {} ({})", member.name, member.endpoint_id);
    match timeout(
      options.connect_timeout,
      request(&endpoint, &member, &options.message),
    )
    .await
    {
      Ok(Ok(response)) => {
        println!("response from {}: {response}", member.name);
        endpoint.close().await;
        return Ok(());
      }
      Ok(Err(err)) => {
        warn!("request to {} failed: {err:#}", member.name);
        last_error = Some(err.to_string());
      }
      Err(_) => {
        let msg = format!("request to {} timed out", member.name);
        warn!("{msg}");
        last_error = Some(msg);
      }
    }
  }

  endpoint.close().await;
  Err(n0_error::anyerr!(
    "no discovered member responded: {}",
    last_error.unwrap_or_else(|| "no members".to_string())
  ))
}

async fn request(endpoint: &Endpoint, member: &MemberRecord, message: &str) -> Result<String> {
  let addr = member.endpoint_addr()?;
  let conn = endpoint.connect(addr, CLUSTER_ALPN).await?;
  let (mut send, mut recv) = conn.open_bi().await.anyerr()?;

  send.write_all(message.as_bytes()).await.anyerr()?;
  send.finish().anyerr()?;

  let response = recv.read_to_end(1024).await.anyerr()?;
  let response = String::from_utf8(response).anyerr()?;
  conn.close(0u32.into(), b"done");
  Ok(response)
}

pub(crate) async fn accept_loop(endpoint: Endpoint) -> Result<()> {
  while let Some(incoming) = endpoint.accept().await {
    let accepting = match incoming.accept() {
      Ok(accepting) => accepting,
      Err(err) => {
        warn!("incoming connection failed: {err:#}");
        continue;
      }
    };

    let me = endpoint.id();
    tokio::spawn(async move {
      if let Err(err) = handle_connection(accepting, me).await {
        warn!("connection handler failed: {err:#}");
      }
    });
  }

  Ok(())
}

async fn handle_connection(mut accepting: iroh::endpoint::Accepting, me: EndpointId) -> Result<()> {
  let alpn = accepting.alpn().await?;
  let conn = accepting.await?;
  let remote = conn.remote_id();
  info!(
    "accepted {} from {}",
    String::from_utf8_lossy(&alpn),
    remote
  );

  let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;
  let message = recv.read_to_end(1024).await.anyerr()?;
  let message = String::from_utf8(message).anyerr()?;
  println!("received from {remote}: {message}");

  let response = format!("hi from {me}; received: {message}");
  send.write_all(response.as_bytes()).await.anyerr()?;
  send.finish().anyerr()?;

  let closed = timeout(Duration::from_secs(3), conn.closed()).await;
  if let Ok(closed) = closed {
    if !matches!(closed, ConnectionError::ApplicationClosed(_)) {
      debug!("remote {remote} closed with: {closed:#}");
    }
  }

  Ok(())
}
