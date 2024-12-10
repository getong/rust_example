use std::{net::SocketAddr, str::FromStr};

use anyhow::{Result, bail};
use clap::Parser;
use futures::StreamExt;
use libp2p::{
  SwarmBuilder,
  core::{Multiaddr, multiaddr::Protocol, upgrade},
  identity::PeerId,
  swarm::{NetworkBehaviour, Swarm, SwarmEvent},
  tcp, tls, yamux,
};
use libp2p_perf::{Final, Intermediate, Run, RunParams, RunUpdate, client, server};
use serde::{Deserialize, Serialize};
use web_time::{Duration, Instant};

#[derive(Debug, Parser)]
#[clap(name = "libp2p perf client")]
struct Opts {
  #[arg(long)]
  server_address: Option<SocketAddr>,
  #[arg(long)]
  transport: Option<Transport>,
  #[arg(long)]
  upload_bytes: Option<usize>,
  #[arg(long)]
  download_bytes: Option<usize>,

  /// Run in server mode.
  #[clap(long)]
  run_server: bool,
}

/// Supported transports by rust-libp2p.
#[derive(Clone, Debug)]
pub enum Transport {
  Tcp,
  QuicV1,
}

impl FromStr for Transport {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    Ok(match s {
      "tcp" => Self::Tcp,
      "quic-v1" => Self::QuicV1,
      other => bail!("unknown transport {other}"),
    })
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let _ = tracing_subscriber::fmt()
    .event_format(
      tracing_subscriber::fmt::format()
        .with_file(true)
        .with_line_number(true),
    )
    .with_ansi(false)
    .with_env_filter("libp2p_perf_example=debug,libp2p_ping=debug")
    .try_init();

  let opts = Opts::parse();
  match opts {
    Opts {
      server_address: Some(server_address),
      transport: None,
      upload_bytes: None,
      download_bytes: None,
      run_server: true,
    } => server(server_address).await?,
    Opts {
      server_address: Some(server_address),
      transport: Some(transport),
      upload_bytes,
      download_bytes,
      run_server: false,
    } => {
      client(server_address, transport, upload_bytes, download_bytes).await?;
    }
    _ => panic!("invalid command line arguments: {opts:?}"),
  };

  Ok(())
}

async fn server(server_address: SocketAddr) -> Result<()> {
  let mut swarm = swarm::<libp2p_perf::server::Behaviour>().await?;

  swarm.listen_on(
    Multiaddr::empty()
      .with(server_address.ip().into())
      .with(Protocol::Tcp(server_address.port())),
  )?;

  swarm
    .listen_on(
      Multiaddr::empty()
        .with(server_address.ip().into())
        .with(Protocol::Udp(server_address.port()))
        .with(Protocol::QuicV1),
    )
    .unwrap();

  tokio::spawn(async move {
    loop {
      match swarm.next().await.unwrap() {
        SwarmEvent::NewListenAddr { address, .. } => {
          tracing::info!(%address, "Listening on address");
        }
        SwarmEvent::IncomingConnection { .. } => {}
        e @ SwarmEvent::IncomingConnectionError { .. } => {
          tracing::error!("{e:?}");
        }
        SwarmEvent::ConnectionEstablished {
          peer_id, endpoint, ..
        } => {
          tracing::info!(peer=%peer_id, ?endpoint, "Established new connection");
        }
        SwarmEvent::ConnectionClosed { .. } => {}
        SwarmEvent::Behaviour(server::Event { .. }) => {
          tracing::info!("Finished run",)
        }
        e => panic!("{e:?}"),
      }
    }
  })
  .await
  .unwrap();

  Ok(())
}

async fn client(
  server_address: SocketAddr,
  transport: Transport,
  upload_bytes: Option<usize>,
  download_bytes: Option<usize>,
) -> Result<()> {
  let server_address = match transport {
    Transport::Tcp => Multiaddr::empty()
      .with(server_address.ip().into())
      .with(Protocol::Tcp(server_address.port())),
    Transport::QuicV1 => Multiaddr::empty()
      .with(server_address.ip().into())
      .with(Protocol::Udp(server_address.port()))
      .with(Protocol::QuicV1),
  };
  let params = RunParams {
    to_send: upload_bytes.unwrap(),
    to_receive: download_bytes.unwrap(),
  };
  let mut swarm = swarm().await?;

  tokio::spawn(async move {
    tracing::info!("start benchmark: custom");

    let start = Instant::now();

    let server_peer_id = connect(&mut swarm, server_address.clone()).await?;

    perf(&mut swarm, server_peer_id, params).await?;

    tracing::info!(
      "{}",
      serde_json::to_string(&BenchmarkResult {
        upload_bytes: params.to_send,
        download_bytes: params.to_receive,
        r#type: "final".to_string(),
        time_seconds: start.elapsed().as_secs_f64(),
      })
      .unwrap()
    );

    anyhow::Ok(())
  })
  .await??;

  Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkResult {
  r#type: String,
  time_seconds: f64,
  upload_bytes: usize,
  download_bytes: usize,
}

async fn swarm<B: NetworkBehaviour + Default>() -> Result<Swarm<B>> {
  let swarm = SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default().nodelay(true),
      tls::Config::new,
      yamux::Config::default,
    )?
    .with_quic()
    .with_dns()?
    .with_behaviour(|_| B::default())?
    .with_swarm_config(|cfg| {
      cfg
        .with_substream_upgrade_protocol_override(upgrade::Version::V1Lazy)
        .with_idle_connection_timeout(Duration::from_secs(60 * 5))
    })
    .build();

  Ok(swarm)
}

async fn connect(
  swarm: &mut Swarm<client::Behaviour>,
  server_address: Multiaddr,
) -> Result<PeerId> {
  let start = Instant::now();
  swarm.dial(server_address.clone()).unwrap();

  let server_peer_id = match swarm.next().await.unwrap() {
    SwarmEvent::ConnectionEstablished { peer_id, .. } => peer_id,
    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
      bail!("Outgoing connection error to {:?}: {:?}", peer_id, error);
    }
    e => panic!("{e:?}"),
  };

  let duration = start.elapsed();
  let duration_seconds = duration.as_secs_f64();

  tracing::info!(elapsed_time=%format!("{duration_seconds:.4} s"));

  Ok(server_peer_id)
}

async fn perf(
  swarm: &mut Swarm<client::Behaviour>,
  server_peer_id: PeerId,
  params: RunParams,
) -> Result<Run> {
  swarm.behaviour_mut().perf(server_peer_id, params)?;

  let duration = loop {
    match swarm.next().await.unwrap() {
      SwarmEvent::Behaviour(client::Event {
        id: _,
        result: Ok(RunUpdate::Intermediate(progressed)),
      }) => {
        tracing::info!("{progressed}");

        let Intermediate {
          duration,
          sent,
          received,
        } = progressed;

        tracing::info!(
          "{}",
          serde_json::to_string(&BenchmarkResult {
            r#type: "intermediate".to_string(),
            time_seconds: duration.as_secs_f64(),
            upload_bytes: sent,
            download_bytes: received,
          })
          .unwrap()
        );
      }
      SwarmEvent::Behaviour(client::Event {
        id: _,
        result: Ok(RunUpdate::Final(Final { duration })),
      }) => break duration,
      e => panic!("{e:?}"),
    };
  };

  let run = Run { params, duration };

  tracing::info!("{run}");

  Ok(run)
}

// copy from rust-libp2p
