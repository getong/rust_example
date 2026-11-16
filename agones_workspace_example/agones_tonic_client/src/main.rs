use std::{collections::HashMap, num::NonZeroUsize, path::PathBuf, time::Duration};

use agones_tonic_client::{
  AllocatorClient, AllocatorConfig,
  allocation::{
    AllocationRequest, GameServerSelector, LabelSelector, MetaPatch, MultiClusterSetting,
    allocation_request::SchedulingStrategy, game_server_selector::GameServerState,
  },
};
use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use futures_util::{StreamExt, stream};

#[derive(Parser)]
#[command(version, about = "Allocate Agones GameServers over gRPC with mTLS")]
struct Args {
  #[arg(long, env = "AGONES_ALLOCATOR_ENDPOINT", help = "HTTPS gRPC origin")]
  endpoint: String,
  #[arg(
    long,
    env = "AGONES_CA_CERT",
    help = "PEM CA that signs the allocator server certificate"
  )]
  ca_cert: PathBuf,
  #[arg(
    long,
    env = "AGONES_CLIENT_CERT",
    help = "PEM client certificate chain"
  )]
  client_cert: PathBuf,
  #[arg(long, env = "AGONES_CLIENT_KEY", help = "PEM client private key")]
  client_key: PathBuf,
  #[arg(
    long,
    env = "AGONES_TLS_SERVER_NAME",
    help = "Expected server certificate SAN/SNI name"
  )]
  server_name: Option<String>,
  #[arg(
    long,
    default_value = "default",
    help = "GameServer namespace; policy namespace in multi-cluster mode"
  )]
  namespace: String,
  #[arg(long, help = "Required agones.dev/fleet label")]
  fleet: String,
  #[arg(long, value_name = "KEY=VALUE", value_parser = parse_pair, help = "Additional GameServer match label; repeatable")]
  selector: Vec<(String, String)>,
  #[arg(long, help = "Enable server-side multi-cluster allocation policies")]
  multi_cluster: bool,
  #[arg(long, value_name = "KEY=VALUE", value_parser = parse_pair, requires = "multi_cluster", help = "Match a GameServerAllocationPolicy label; repeatable")]
  policy_label: Vec<(String, String)>,
  #[arg(long, value_enum, default_value = "packed")]
  scheduling: Scheduling,
  #[arg(long, value_name = "KEY=VALUE", value_parser = parse_pair, help = "Patch allocated GameServer label; repeatable")]
  label: Vec<(String, String)>,
  #[arg(long, value_name = "KEY=VALUE", value_parser = parse_pair, help = "Patch allocated GameServer annotation; repeatable")]
  annotation: Vec<(String, String)>,
  #[arg(long, default_value = "5000", value_parser = clap::value_parser!(u64).range(1..))]
  connect_timeout_ms: u64,
  #[arg(long, default_value = "5000", value_parser = clap::value_parser!(u64).range(1..), help = "Total per-allocation deadline, including permit queue time")]
  timeout_ms: u64,
  #[arg(
    long,
    default_value = "32",
    help = "Maximum concurrent allocations sharing one channel"
  )]
  max_in_flight: NonZeroUsize,
  #[arg(
    long,
    default_value = "1",
    help = "Number of distinct allocations to make, not a retry count"
  )]
  count: NonZeroUsize,
}

#[derive(Clone, Copy, ValueEnum)]
enum Scheduling {
  Packed,
  Distributed,
}

fn parse_pair(value: &str) -> Result<(String, String), String> {
  let (key, value) = value
    .split_once('=')
    .ok_or_else(|| "expected KEY=VALUE".to_string())?;
  if key.trim().is_empty() {
    return Err("key must not be empty".into());
  }
  Ok((key.into(), value.into()))
}

impl Args {
  fn request(&self) -> Result<AllocationRequest> {
    if self.namespace.trim().is_empty() || self.fleet.trim().is_empty() {
      bail!("namespace and fleet must not be empty");
    }
    let mut match_labels: HashMap<_, _> = self.selector.iter().cloned().collect();
    if match_labels
      .get("agones.dev/fleet")
      .is_some_and(|fleet| fleet != &self.fleet)
    {
      bail!("--selector agones.dev/fleet must match --fleet");
    }
    match_labels.insert("agones.dev/fleet".into(), self.fleet.clone());

    Ok(AllocationRequest {
      namespace: self.namespace.clone(),
      game_server_selectors: vec![GameServerSelector {
        match_labels,
        game_server_state: GameServerState::Ready.into(),
        ..Default::default()
      }],
      scheduling: match self.scheduling {
        Scheduling::Packed => SchedulingStrategy::Packed.into(),
        Scheduling::Distributed => SchedulingStrategy::Distributed.into(),
      },
      metadata: if self.label.is_empty() && self.annotation.is_empty() {
        None
      } else {
        Some(MetaPatch {
          labels: self.label.iter().cloned().collect(),
          annotations: self.annotation.iter().cloned().collect(),
        })
      },
      multi_cluster_setting: self.multi_cluster.then(|| MultiClusterSetting {
        enabled: true,
        policy_selector: Some(LabelSelector {
          match_labels: self.policy_label.iter().cloned().collect(),
        }),
      }),
      ..Default::default()
    })
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();
  let request = args.request()?;
  let mut config = AllocatorConfig::new(
    &args.endpoint,
    &args.ca_cert,
    &args.client_cert,
    &args.client_key,
  );
  config.server_name = args.server_name;
  config.connect_timeout = Duration::from_millis(args.connect_timeout_ms);
  config.request_timeout = Duration::from_millis(args.timeout_ms);
  config.max_in_flight = args.max_in_flight.get();
  let client = AllocatorClient::connect(config)
    .await
    .context("failed to connect to Agones Allocator")?;

  let mut allocations = stream::iter(0 .. args.count.get())
    .map(|index| {
      let client = client.clone();
      let request = request.clone();
      async move { (index + 1, client.allocate(request).await) }
    })
    .buffer_unordered(args.max_in_flight.get());

  let mut failures = 0;
  while let Some((index, result)) = allocations.next().await {
    match result {
      Ok(response) => println!("{}", serde_json::to_string(&response)?),
      Err(error) => {
        failures += 1;
        eprintln!("allocation {index} failed: {error}");
      }
    }
  }
  if failures != 0 {
    bail!(
      "{failures}/{} allocations failed; successful allocations are not rolled back; do not \
       blindly retry uncertain outcomes",
      args.count
    );
  }
  Ok(())
}
