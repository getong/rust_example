use std::{
  net::{Ipv4Addr, SocketAddr},
  time::Duration,
};

use mainline::Testnet;
use n0_error::{Result, StdResultExt};

use crate::{
  dht::{build_dht, local_dht_options, publish_member},
  endpoint::build_endpoint,
  identity::ClusterIdentity,
  options::{ClientOptions, IrohOptions, LocalDemoOptions},
  protocols::REQUEST_PROTOCOL,
  records::member_from_endpoint,
  request::{accept_loop, run_client},
};

pub async fn run_local_demo(options: LocalDemoOptions) -> Result<()> {
  let testnet = Testnet::builder(options.dht_nodes)
    .bind_address(Ipv4Addr::LOCALHOST)
    .build()
    .anyerr()?;

  println!("local mainline bootstrap: {}", testnet.bootstrap.join(" "));

  let cluster = ClusterIdentity::default();
  let mut server_tasks = Vec::with_capacity(options.servers);
  let mut server_endpoints = Vec::with_capacity(options.servers);

  for index in 0..options.servers {
    let dht = build_dht(&local_dht_options(testnet.bootstrap.clone()))?.as_async();

    let endpoint = build_endpoint(&local_iroh_options(true), true).await?;

    let name = format!("demo-server-{}", index + 1);
    publish_member(
      &dht,
      &cluster,
      member_from_endpoint(&endpoint, &name, &[REQUEST_PROTOCOL]),
      16,
    )
    .await?;

    let endpoint_for_task = endpoint.clone();
    server_tasks.push(tokio::spawn(async move {
      if let Err(err) = accept_loop(endpoint_for_task).await {
        eprintln!("demo accept loop stopped: {err:#}");
      }
    }));
    server_endpoints.push(endpoint);
  }

  let client = ClientOptions {
    cluster,
    dht: local_dht_options(testnet.bootstrap.clone()),
    iroh: local_iroh_options(false),
    message: options.message,
    discover_timeout: options.discover_timeout,
    connect_timeout: Duration::from_secs(8),
  };

  let result = run_client(client).await;

  for endpoint in server_endpoints {
    endpoint.close().await;
  }
  for task in server_tasks {
    task.abort();
  }

  result
}

fn local_iroh_options(_accept: bool) -> IrohOptions {
  IrohOptions {
    bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
    relay: false,
    wait_online: Duration::from_secs(0),
  }
}
