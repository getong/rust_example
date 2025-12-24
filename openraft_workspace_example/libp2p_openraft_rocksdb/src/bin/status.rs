use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use libp2p::{
  Multiaddr, StreamProtocol, gossipsub, identity,
  kad::{self, store::MemoryStore},
  mdns, noise, ping,
  request_response::{self, ProtocolSupport},
  tcp, tls, yamux,
};
use libp2p_openraft_rocksdb::{
  app,
  network::{
    proto_codec::{ProstCodec, ProtoCodec},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    swarm::{Behaviour, Libp2pClient, run_swarm_client_with_shutdown},
    transport::parse_p2p_addr,
  },
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  signal,
};
use tokio::sync::mpsc;

#[derive(Parser, Debug, Clone)]
#[command(
  author,
  version,
  about = "Query OpenRaft metrics (leader/term) via libp2p RPC"
)]
pub struct Opt {
  /// Target node multiaddr including /p2p/<peerid>
  #[arg(long)]
  pub addr: String,

  /// Optional libp2p identity (protobuf). If absent, uses an ephemeral key.
  #[arg(long)]
  pub key: Option<std::path::PathBuf>,

  /// RPC timeout seconds
  #[arg(long, default_value_t = 5)]
  pub timeout_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();

  let opt = Opt::parse();

  let (peer, maddr) = parse_p2p_addr(&opt.addr).context("invalid --addr")?;

  let local_key = match &opt.key {
    Some(p) => app::load_or_create_keypair(p)?,
    None => identity::Keypair::generate_ed25519(),
  };

  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      (tls::Config::new, noise::Config::new),
      yamux::Config::default,
    )
    .context("build tcp/noise/yamux")?
    .with_quic()
    .with_behaviour(|key| {
      let cfg = request_response::Config::default();
      let peer_id = libp2p::PeerId::from(key.public());
      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
      let mut kad = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));
      kad.set_mode(Some(kad::Mode::Client));
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .build()
        .map_err(|e| anyhow::anyhow!("gossipsub config error: {e}"))?;
      let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
      )
      .map_err(|e| anyhow::anyhow!("gossipsub init error: {e}"))?;
      let ping = ping::Behaviour::new(ping::Config::new());

      Ok(Behaviour {
        raft: request_response::Behaviour::with_codec(
          ProtoCodec::default(),
          [(
            StreamProtocol::new("/openraft/raft/1"),
            ProtocolSupport::Full,
          )],
          cfg.clone(),
        ),
        kv: request_response::Behaviour::with_codec(
          ProstCodec::<RaftKvRequest, RaftKvResponse>::default(),
          [(StreamProtocol::new("/openraft/kv/1"), ProtocolSupport::Full)],
          cfg,
        ),
        gossipsub,
        ping,
        mdns,
        kad,
      })
    })
    .context("build behaviour")?
    .build();

  // Client-only tool: no need to listen on a fixed port.
  // Still, listening on an ephemeral port helps with NAT-less local testing.
  let listen: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().expect("static addr");
  let _ = swarm.listen_on(listen);

  let (cmd_tx, cmd_rx) = mpsc::channel(64);
  let client = Libp2pClient::new(cmd_tx, Duration::from_secs(opt.timeout_secs));
  let shutdown = signal::spawn_handler();
  tokio::spawn(run_swarm_client_with_shutdown(
    swarm,
    cmd_rx,
    shutdown.shutdown_rx(),
  ));

  client.dial(maddr.clone()).await;

  // Small delay to allow the dial to happen; libp2p will also dial implicitly.
  tokio::time::sleep(Duration::from_millis(200)).await;

  let resp = client
    .request(peer, RaftRpcRequest::GetMetrics)
    .await
    .context("rpc get-metrics")?;

  match resp {
    RaftRpcResponse::GetMetrics(metrics) => {
      // Best-effort extraction; always print full metrics for debugging.
      println!("current_leader: {:?}", metrics.current_leader);
      println!("state: {:?}", metrics.state);
      println!("vote: {:?}", metrics.vote);
      println!("metrics: {:#?}", metrics);
      Ok(())
    }
    RaftRpcResponse::Error(e) => anyhow::bail!("remote error: {e}"),
    other => anyhow::bail!("unexpected response: {other:?}"),
  }
}
