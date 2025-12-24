use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use libp2p::{
  Multiaddr, StreamProtocol, identity,
  kad::{self, store::MemoryStore},
  mdns, noise,
  request_response::{self, ProtocolSupport},
  tcp, tls, yamux,
};
use libp2p_openraft_rocksdb::{
  app,
  network::{
    proto_codec::{ProstCodec, ProtoCodec},
    swarm::{Behaviour, KvClient, run_swarm_client},
    transport::parse_p2p_addr,
  },
  proto::raft_kv::{
    DeleteValueRequest, GetValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest, raft_kv_request::Op as KvRequestOp, raft_kv_response::Op as KvResponseOp,
  },
};
use tokio::sync::mpsc;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "KV operations via libp2p protobuf protocol")]
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

  #[command(subcommand)]
  pub cmd: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
  Get { key: String },
  Set { key: String, value: String },
  Update { key: String, value: String },
  Delete { key: String },
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
        mdns,
        kad,
      })
    })
    .context("build behaviour")?
    .build();

  let listen: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().expect("static addr");
  let _ = swarm.listen_on(listen);

  let (cmd_tx, cmd_rx) = mpsc::channel(64);
  let client = KvClient::new(cmd_tx, Duration::from_secs(opt.timeout_secs));
  tokio::spawn(run_swarm_client(swarm, cmd_rx));

  client.dial(maddr.clone()).await;
  tokio::time::sleep(Duration::from_millis(200)).await;

  let req = match opt.cmd {
    Command::Get { key } => RaftKvRequest {
      op: Some(KvRequestOp::Get(GetValueRequest { key })),
    },
    Command::Set { key, value } => RaftKvRequest {
      op: Some(KvRequestOp::Set(SetValueRequest { key, value })),
    },
    Command::Update { key, value } => RaftKvRequest {
      op: Some(KvRequestOp::Update(UpdateValueRequest { key, value })),
    },
    Command::Delete { key } => RaftKvRequest {
      op: Some(KvRequestOp::Delete(DeleteValueRequest { key })),
    },
  };

  let resp = client.request(peer, req).await.context("kv request")?;

  match resp.op {
    Some(KvResponseOp::Get(resp)) => {
      println!("found: {}, value: {}", resp.found, resp.value);
    }
    Some(KvResponseOp::Set(resp)) => {
      println!("ok: {}, value: {}", resp.ok, resp.value);
    }
    Some(KvResponseOp::Update(resp)) => {
      println!("ok: {}, value: {}", resp.ok, resp.value);
    }
    Some(KvResponseOp::Delete(resp)) => {
      println!("ok: {}", resp.ok);
    }
    Some(KvResponseOp::Error(resp)) => {
      println!("error: {}", resp.message);
    }
    None => {
      println!("error: empty response");
    }
  }

  Ok(())
}
