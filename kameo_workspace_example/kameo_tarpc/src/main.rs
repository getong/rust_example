mod gen_server_actor;
mod raft_counter;

use std::{future::Future, net::SocketAddr, time::Duration};

use anyhow::{Context as _, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use futures::{StreamExt, future};
use kameo::{prelude::*, remote};
use libp2p::{
  Multiaddr, PeerId, SwarmBuilder,
  multiaddr::Protocol,
  noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use prost::Message as ProstMessage;
use serde::{Deserialize, Serialize};
use tarpc::{
  client, context,
  server::{self, Channel},
  tokio_serde::formats::Json,
};
use tokio::time::sleep;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::raft_counter::{CounterRaftHandle, SharedCounterRaft};

mod mypackage {
  include!(concat!(env!("OUT_DIR"), "/mypackage.rs"));
}

const DEFAULT_ACTOR_NAME: &str = "distributed-counter";

#[derive(Parser, Debug)]
#[command(
  name = "kameo_tarpc",
  about = "Distributed tarpc RPC forwarding to a remote kameo actor over libp2p."
)]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
  /// Starts a distributed kameo actor node.
  ActorNode(NodeArgs),
  /// Starts a tarpc server and forwards each RPC to the remote actor node.
  RpcServer(RpcServerArgs),
  /// Calls the tarpc server.
  RpcClient {
    #[arg(long, default_value = "127.0.0.1:7000")]
    server_addr: SocketAddr,
    #[arg(long, value_enum, default_value = "add")]
    operation: CounterOperation,
    #[arg(long)]
    amount: u32,
    #[arg(long, default_value = "demo-client")]
    caller: String,
  },
  /// Encodes a protobuf message and forwards it through tarpc to the remote actor.
  RpcProstClient {
    #[arg(long, default_value = "127.0.0.1:7000")]
    server_addr: SocketAddr,
    #[arg(long, value_enum, default_value = "my-message")]
    kind: ProstMessageKind,
    #[arg(long)]
    payload: String,
    #[arg(long, default_value = "demo-prost-client")]
    caller: String,
  },
  /// Runs a local kameo actor that uses tokio::select! like an Erlang gen_server loop.
  GenServerDemo,
}

#[derive(Parser, Debug, Clone)]
struct NodeArgs {
  #[arg(long, default_value = DEFAULT_ACTOR_NAME)]
  actor_name: String,
  #[arg(long, default_value = "/ip4/127.0.0.1/tcp/4101")]
  swarm_listen_addr: Multiaddr,
  #[arg(long, default_value = "./data/actor-node")]
  raft_db_path: String,
  #[arg(long = "seed")]
  seed_addrs: Vec<Multiaddr>,
}

#[derive(Parser, Debug, Clone)]
struct RpcServerArgs {
  #[command(flatten)]
  node: NodeArgs,
  #[arg(long, default_value = "127.0.0.1:7000")]
  rpc_listen_addr: SocketAddr,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CounterOperation {
  Add,
  Subtract,
}

impl CounterOperation {
  fn as_str(self) -> &'static str {
    match self {
      Self::Add => "add",
      Self::Subtract => "subtract",
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum)]
enum ProstMessageKind {
  MyMessage,
  OtherMessage,
}

impl ProstMessageKind {
  fn as_str(self) -> &'static str {
    match self {
      Self::MyMessage => "my-message",
      Self::OtherMessage => "other-message",
    }
  }

  fn field_name(self) -> &'static str {
    match self {
      Self::MyMessage => "content",
      Self::OtherMessage => "data",
    }
  }
}

#[derive(Actor, RemoteActor)]
struct CounterActor {
  raft: SharedCounterRaft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Add {
  amount: u32,
  caller: String,
  origin_peer: PeerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Subtract {
  amount: u32,
  caller: String,
  origin_peer: PeerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProstCounterMessage {
  kind: ProstMessageKind,
  payload: Vec<u8>,
  caller: String,
  origin_peer: PeerId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
struct CounterSnapshot {
  actor_id: String,
  handled_by_peer: String,
  total: i64,
  acknowledged_caller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
struct ProstCounterReply {
  actor_id: String,
  handled_by_peer: String,
  message_kind: ProstMessageKind,
  field_name: String,
  value: String,
  acknowledged_caller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ProstCounterError {
  DecodeFailed {
    kind: ProstMessageKind,
    message: String,
  },
}

impl std::fmt::Display for ProstCounterError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DecodeFailed { kind, message } => {
        write!(
          f,
          "failed to decode protobuf {} payload: {message}",
          kind.as_str()
        )
      }
    }
  }
}

#[remote_message]
impl Message<Add> for CounterActor {
  type Reply = CounterSnapshot;

  async fn handle(&mut self, msg: Add, ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self
      .handle_counter_operation(CounterOperation::Add, msg.amount, msg.caller, ctx)
      .await
  }
}

#[remote_message]
impl Message<Subtract> for CounterActor {
  type Reply = CounterSnapshot;

  async fn handle(&mut self, msg: Subtract, ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    self
      .handle_counter_operation(CounterOperation::Subtract, msg.amount, msg.caller, ctx)
      .await
  }
}

#[remote_message]
impl Message<ProstCounterMessage> for CounterActor {
  type Reply = Result<ProstCounterReply, ProstCounterError>;

  async fn handle(
    &mut self,
    msg: ProstCounterMessage,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let ProstCounterMessage {
      kind,
      payload,
      caller,
      origin_peer,
    } = msg;

    match kind {
      ProstMessageKind::MyMessage => {
        let decoded = decode_prost_payload::<mypackage::MyMessage>(kind, &payload)?;
        Ok(self.handle_my_message(decoded, caller, origin_peer, ctx))
      }
      ProstMessageKind::OtherMessage => {
        let decoded = decode_prost_payload::<mypackage::OtherMessage>(kind, &payload)?;
        Ok(self.handle_other_message(decoded, caller, origin_peer, ctx))
      }
    }
  }
}

impl CounterActor {
  async fn handle_counter_operation(
    &mut self,
    operation: CounterOperation,
    amount: u32,
    caller: String,
    ctx: &mut Context<Self, CounterSnapshot>,
  ) -> CounterSnapshot {
    let raft = self.raft.lock().await;
    let previous_total = raft.current_total().await.unwrap_or(0);
    let applied = match operation {
      CounterOperation::Add => raft.add_and_get_total(amount, caller.clone()).await,
      CounterOperation::Subtract => raft.subtract_and_get_total(amount, caller.clone()).await,
    }
    .unwrap_or_else(|err| {
      warn!(
        "raft apply failed operation={} caller={} amount={} error={err}",
        operation.as_str(),
        caller,
        amount
      );
      crate::raft_counter::CounterReply {
        total: previous_total,
        caller: caller.clone(),
      }
    });
    let new_total = applied.total;
    info!(
      "counter actor handled operation={} caller={} amount={} previous_total={} new_total={} \
       actor_id={}",
      operation.as_str(),
      caller,
      amount,
      previous_total,
      new_total,
      ctx.actor_ref().id()
    );
    CounterSnapshot {
      actor_id: ctx.actor_ref().id().to_string(),
      handled_by_peer: ctx
        .actor_ref()
        .id()
        .peer_id()
        .map(ToString::to_string)
        .unwrap_or_else(|| "local".to_string()),
      total: new_total,
      acknowledged_caller: applied.caller,
    }
  }

  fn handle_my_message(
    &mut self,
    msg: mypackage::MyMessage,
    caller: String,
    origin_peer: PeerId,
    ctx: &mut Context<Self, Result<ProstCounterReply, ProstCounterError>>,
  ) -> ProstCounterReply {
    self.handle_decoded_prost_message(
      ProstMessageKind::MyMessage,
      msg.content,
      caller,
      origin_peer,
      ctx,
    )
  }

  fn handle_other_message(
    &mut self,
    msg: mypackage::OtherMessage,
    caller: String,
    origin_peer: PeerId,
    ctx: &mut Context<Self, Result<ProstCounterReply, ProstCounterError>>,
  ) -> ProstCounterReply {
    self.handle_decoded_prost_message(
      ProstMessageKind::OtherMessage,
      msg.data,
      caller,
      origin_peer,
      ctx,
    )
  }

  fn handle_decoded_prost_message(
    &mut self,
    kind: ProstMessageKind,
    value: String,
    caller: String,
    origin_peer: PeerId,
    ctx: &mut Context<Self, Result<ProstCounterReply, ProstCounterError>>,
  ) -> ProstCounterReply {
    let actor_id = ctx.actor_ref().id().to_string();
    let handled_by_peer = ctx
      .actor_ref()
      .id()
      .peer_id()
      .map(ToString::to_string)
      .unwrap_or_else(|| "local".to_string());
    info!(
      "counter actor handled prost message kind={} caller={} origin_peer={} field={} value={} \
       actor_id={}",
      kind.as_str(),
      caller,
      origin_peer,
      kind.field_name(),
      value,
      actor_id
    );

    ProstCounterReply {
      actor_id,
      handled_by_peer,
      message_kind: kind,
      field_name: kind.field_name().to_string(),
      value,
      acknowledged_caller: caller,
    }
  }
}

#[tarpc::service]
trait CounterRpc {
  async fn add(amount: u32, caller: String) -> Result<RpcCounterResponse, RpcError>;
  async fn subtract(amount: u32, caller: String) -> Result<RpcCounterResponse, RpcError>;
  async fn process_prost_message(
    kind: ProstMessageKind,
    payload: Vec<u8>,
    caller: String,
  ) -> Result<RpcProstResponse, RpcError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcCounterResponse {
  actor_registration: String,
  actor_id: String,
  actor_peer: String,
  rpc_server_peer: String,
  total: i64,
  caller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcProstResponse {
  actor_registration: String,
  actor_id: String,
  actor_peer: String,
  rpc_server_peer: String,
  message_kind: ProstMessageKind,
  field_name: String,
  value: String,
  caller: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum RpcError {
  RemoteActorUnavailable { message: String },
}

#[derive(NetworkBehaviour)]
struct KameoBehaviour {
  kameo: remote::Behaviour,
}

#[derive(Clone)]
struct CounterRpcServer {
  actor_name: String,
  local_peer_id: PeerId,
}

impl CounterRpcServer {
  async fn apply_counter_operation(
    &self,
    operation: CounterOperation,
    amount: u32,
    caller: String,
  ) -> Result<RpcCounterResponse, RpcError> {
    let snapshot = forward_to_counter(
      &self.actor_name,
      self.local_peer_id,
      operation,
      amount,
      caller.clone(),
    )
    .await
    .map_err(|err| RpcError::RemoteActorUnavailable {
      message: err.to_string(),
    })?;

    Ok(RpcCounterResponse {
      actor_registration: self.actor_name.clone(),
      actor_id: snapshot.actor_id,
      actor_peer: snapshot.handled_by_peer,
      rpc_server_peer: self.local_peer_id.to_string(),
      total: snapshot.total,
      caller: snapshot.acknowledged_caller,
    })
  }

  async fn apply_prost_message(
    &self,
    kind: ProstMessageKind,
    payload: Vec<u8>,
    caller: String,
  ) -> Result<RpcProstResponse, RpcError> {
    let reply = forward_prost_to_counter(
      &self.actor_name,
      self.local_peer_id,
      kind,
      payload,
      caller.clone(),
    )
    .await
    .map_err(|err| RpcError::RemoteActorUnavailable {
      message: err.to_string(),
    })?;

    Ok(RpcProstResponse {
      actor_registration: self.actor_name.clone(),
      actor_id: reply.actor_id,
      actor_peer: reply.handled_by_peer,
      rpc_server_peer: self.local_peer_id.to_string(),
      message_kind: reply.message_kind,
      field_name: reply.field_name,
      value: reply.value,
      caller: reply.acknowledged_caller,
    })
  }
}

impl CounterRpc for CounterRpcServer {
  async fn add(
    self,
    _: context::Context,
    amount: u32,
    caller: String,
  ) -> Result<RpcCounterResponse, RpcError> {
    self
      .apply_counter_operation(CounterOperation::Add, amount, caller)
      .await
  }

  async fn subtract(
    self,
    _: context::Context,
    amount: u32,
    caller: String,
  ) -> Result<RpcCounterResponse, RpcError> {
    self
      .apply_counter_operation(CounterOperation::Subtract, amount, caller)
      .await
  }

  async fn process_prost_message(
    self,
    _: context::Context,
    kind: ProstMessageKind,
    payload: Vec<u8>,
    caller: String,
  ) -> Result<RpcProstResponse, RpcError> {
    self.apply_prost_message(kind, payload, caller).await
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  init_tracing()?;

  match Cli::parse().command {
    Command::ActorNode(args) => run_actor_node(args).await,
    Command::RpcServer(args) => run_rpc_server(args).await,
    Command::RpcClient {
      server_addr,
      operation,
      amount,
      caller,
    } => run_rpc_client(server_addr, operation, amount, caller).await,
    Command::RpcProstClient {
      server_addr,
      kind,
      payload,
      caller,
    } => run_rpc_prost_client(server_addr, kind, payload, caller).await,
    Command::GenServerDemo => gen_server_actor::run_gen_server_demo().await,
  }
}

async fn run_actor_node(args: NodeArgs) -> anyhow::Result<()> {
  let local_peer_id = start_swarm(&args.swarm_listen_addr, &args.seed_addrs).await?;
  info!(
    "actor node ready peer_id={local_peer_id} actor_name={} listen_addr={}",
    args.actor_name, args.swarm_listen_addr
  );

  let raft = CounterRaftHandle::open_single_node(1_u64.into(), &args.raft_db_path).await?;
  let actor_ref = CounterActor::spawn(CounterActor {
    raft: std::sync::Arc::new(tokio::sync::Mutex::new(raft)),
  });
  let actor_id = actor_ref.id();
  actor_ref
    .register(args.actor_name.clone())
    .await
    .with_context(|| format!("failed to register actor as {}", args.actor_name))?;

  info!(
    "registered remote actor actor_name={} actor_id={actor_id}",
    args.actor_name
  );
  wait_for_ctrl_c().await
}

async fn run_rpc_server(args: RpcServerArgs) -> anyhow::Result<()> {
  let local_peer_id = start_swarm(&args.node.swarm_listen_addr, &args.node.seed_addrs).await?;
  info!(
    "rpc server swarm ready peer_id={local_peer_id} actor_name={} listen_addr={}",
    args.node.actor_name, args.node.swarm_listen_addr
  );

  let mut listener = tarpc::serde_transport::tcp::listen(&args.rpc_listen_addr, Json::default)
    .await
    .with_context(|| format!("failed to listen tarpc on {}", args.rpc_listen_addr))?;
  listener.config_mut().max_frame_length(usize::MAX);

  info!("tarpc listening on {}", args.rpc_listen_addr);

  listener
    .filter_map(|result| future::ready(result.ok()))
    .map(server::BaseChannel::with_defaults)
    .map(move |channel| {
      let remote = channel.transport().peer_addr().ok();
      let server_impl = CounterRpcServer {
        actor_name: args.node.actor_name.clone(),
        local_peer_id,
      };
      async move {
        if let Some(remote) = remote {
          info!("accepted tarpc client={remote}");
        }
        channel
          .execute(server_impl.serve())
          .for_each(spawn_task)
          .await;
      }
    })
    .buffer_unordered(32)
    .for_each(|_| async {})
    .await;

  Ok(())
}

async fn run_rpc_client(
  server_addr: SocketAddr,
  operation: CounterOperation,
  amount: u32,
  caller: String,
) -> anyhow::Result<()> {
  let mut transport = tarpc::serde_transport::tcp::connect(server_addr, Json::default);
  transport.config_mut().max_frame_length(usize::MAX);

  let client = CounterRpcClient::new(
    client::Config::default(),
    transport
      .await
      .with_context(|| format!("failed to connect tarpc server {server_addr}"))?,
  )
  .spawn();

  let rpc_result = match operation {
    CounterOperation::Add => client
      .add(context::current(), amount, caller.clone())
      .await
      .context("tarpc add rpc failed")?,
    CounterOperation::Subtract => client
      .subtract(context::current(), amount, caller.clone())
      .await
      .context("tarpc subtract rpc failed")?,
  };
  let reply = rpc_result.map_err(|err| anyhow!("rpc server returned error: {err:?}"))?;

  println!(
    "rpc_client operation={} caller={} amount={} total={} actor_registration={} actor_id={} \
     actor_peer={} rpc_server_peer={}",
    operation.as_str(),
    reply.caller,
    amount,
    reply.total,
    reply.actor_registration,
    reply.actor_id,
    reply.actor_peer,
    reply.rpc_server_peer
  );

  Ok(())
}

async fn run_rpc_prost_client(
  server_addr: SocketAddr,
  kind: ProstMessageKind,
  payload: String,
  caller: String,
) -> anyhow::Result<()> {
  let mut transport = tarpc::serde_transport::tcp::connect(server_addr, Json::default);
  transport.config_mut().max_frame_length(usize::MAX);

  let client = CounterRpcClient::new(
    client::Config::default(),
    transport
      .await
      .with_context(|| format!("failed to connect tarpc server {server_addr}"))?,
  )
  .spawn();

  let encoded_payload = encode_prost_payload(kind, payload);
  let rpc_result = client
    .process_prost_message(context::current(), kind, encoded_payload, caller.clone())
    .await
    .context("tarpc process_prost_message rpc failed")?;
  let reply = rpc_result.map_err(|err| anyhow!("rpc server returned error: {err:?}"))?;

  println!(
    "rpc_prost_client kind={} caller={} field={} value={} actor_registration={} actor_id={} \
     actor_peer={} rpc_server_peer={}",
    reply.message_kind.as_str(),
    reply.caller,
    reply.field_name,
    reply.value,
    reply.actor_registration,
    reply.actor_id,
    reply.actor_peer,
    reply.rpc_server_peer
  );

  Ok(())
}

async fn start_swarm(listen_addr: &Multiaddr, seed_addrs: &[Multiaddr]) -> anyhow::Result<PeerId> {
  let mut swarm = SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default().nodelay(true),
      noise::Config::new,
      yamux::Config::default,
    )
    .map_err(|err| anyhow!("failed to configure tcp transport: {err}"))?
    .with_behaviour(|key| {
      let local_peer_id = key.public().to_peer_id();
      let behaviour = remote::Behaviour::new(
        local_peer_id,
        remote::messaging::Config::default()
          .with_request_timeout(Duration::from_secs(10))
          .with_max_concurrent_streams(128),
      );
      Ok(KameoBehaviour { kameo: behaviour })
    })
    .map_err(|err| anyhow!("failed to build kameo behaviour: {err}"))?
    .build();

  swarm
    .behaviour()
    .kameo
    .try_init_global()
    .map_err(|_| anyhow!("kameo actor swarm already bootstrapped in this process"))?;

  swarm
    .listen_on(listen_addr.clone())
    .map_err(|err| anyhow!("failed to listen on {listen_addr}: {err:?}"))?;

  for seed in seed_addrs {
    if let Some((peer_id, addr)) = split_seed_addr(seed.clone()) {
      swarm.add_peer_address(peer_id, addr.clone());
      info!("registered seed peer_id={peer_id} addr={addr} in swarm address book");
    }
    info!("dialing seed address {seed}");
    swarm
      .dial(seed.clone())
      .map_err(|err| anyhow!("failed to dial seed {seed}: {err:?}"))?;
  }

  let local_peer_id = *swarm.local_peer_id();
  tokio::spawn(run_swarm_event_loop(swarm));

  wait_for_initial_connections().await;
  Ok(local_peer_id)
}

async fn run_swarm_event_loop(mut swarm: libp2p::Swarm<KameoBehaviour>) {
  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr { address, .. } => {
        info!("kameo swarm listening on {address}");
      }
      SwarmEvent::ConnectionEstablished {
        peer_id, endpoint, ..
      } => {
        info!(
          "kameo swarm connected peer_id={peer_id} remote_addr={}",
          endpoint.get_remote_address()
        );
      }
      SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
        warn!("kameo swarm disconnected peer_id={peer_id} cause={cause:?}");
      }
      SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
        warn!("kameo swarm dial failure peer_id={peer_id:?} error={error}");
      }
      SwarmEvent::IncomingConnectionError { error, .. } => {
        warn!("kameo swarm incoming connection error={error}");
      }
      SwarmEvent::Behaviour(KameoBehaviourEvent::Kameo(event)) => {
        info!("kameo swarm event={event:?}");
      }
      _ => {}
    }
  }
}

async fn wait_for_initial_connections() {
  sleep(Duration::from_millis(500)).await;
}

fn split_seed_addr(addr: Multiaddr) -> Option<(PeerId, Multiaddr)> {
  let mut base = Multiaddr::empty();
  let mut peer_id = None;

  for protocol in &addr {
    match protocol {
      Protocol::P2p(id) => peer_id = Some(id),
      other => base.push(other.acquire()),
    }
  }

  peer_id.map(|id| (id, base))
}

async fn wait_for_ctrl_c() -> anyhow::Result<()> {
  tokio::signal::ctrl_c()
    .await
    .context("failed to wait for ctrl-c")?;
  Ok(())
}

async fn forward_to_counter(
  actor_name: &str,
  local_peer_id: PeerId,
  operation: CounterOperation,
  amount: u32,
  caller: String,
) -> anyhow::Result<CounterSnapshot> {
  let remote_actor = lookup_counter(actor_name, local_peer_id).await?;
  let target_actor_id = remote_actor.id();
  info!(
    "forwarding rpc operation={} to actor_name={actor_name} actor_id={target_actor_id}",
    operation.as_str()
  );

  match operation {
    CounterOperation::Add => {
      remote_actor
        .ask(&Add {
          amount,
          caller,
          origin_peer: local_peer_id,
        })
        .send()
        .await
    }
    CounterOperation::Subtract => {
      remote_actor
        .ask(&Subtract {
          amount,
          caller,
          origin_peer: local_peer_id,
        })
        .send()
        .await
    }
  }
  .map_err(|err| anyhow!("remote actor call failed: {err}"))
}

async fn forward_prost_to_counter(
  actor_name: &str,
  local_peer_id: PeerId,
  kind: ProstMessageKind,
  payload: Vec<u8>,
  caller: String,
) -> anyhow::Result<ProstCounterReply> {
  let remote_actor = lookup_counter(actor_name, local_peer_id).await?;
  let target_actor_id = remote_actor.id();
  info!(
    "forwarding protobuf message kind={} to actor_name={actor_name} actor_id={target_actor_id}",
    kind.as_str()
  );

  remote_actor
    .ask(&ProstCounterMessage {
      kind,
      payload,
      caller,
      origin_peer: local_peer_id,
    })
    .send()
    .await
    .map_err(|err| anyhow!("remote actor protobuf call failed: {err}"))
}

async fn lookup_counter(
  actor_name: &str,
  local_peer_id: PeerId,
) -> anyhow::Result<RemoteActorRef<CounterActor>> {
  for attempt in 1 ..= 15 {
    match RemoteActorRef::<CounterActor>::lookup(actor_name)
      .await
      .with_context(|| format!("lookup failed for actor name {actor_name}"))?
    {
      Some(remote_actor) if remote_actor.id().peer_id() != Some(&local_peer_id) => {
        return Ok(remote_actor);
      }
      Some(_) => {
        warn!("lookup found only local peer on attempt={attempt}, retrying");
      }
      None => {
        warn!("lookup returned no actor on attempt={attempt}, retrying");
      }
    }
    sleep(Duration::from_millis(800)).await;
  }

  Err(anyhow!(
    "unable to find remote actor '{actor_name}' from peer {local_peer_id}"
  ))
}

fn init_tracing() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(
      EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,kameo_tarpc=info")),
    )
    .with_target(false)
    .without_time()
    .try_init()
    .map_err(|err| anyhow!("failed to initialize tracing: {err}"))
}

fn decode_prost_payload<M>(kind: ProstMessageKind, payload: &[u8]) -> Result<M, ProstCounterError>
where
  M: ProstMessage + Default,
{
  M::decode(payload).map_err(|err| ProstCounterError::DecodeFailed {
    kind,
    message: err.to_string(),
  })
}

fn encode_prost_payload(kind: ProstMessageKind, payload: String) -> Vec<u8> {
  match kind {
    ProstMessageKind::MyMessage => {
      let msg = mypackage::MyMessage { content: payload };
      msg.encode_to_vec()
    }
    ProstMessageKind::OtherMessage => {
      let msg = mypackage::OtherMessage { data: payload };
      msg.encode_to_vec()
    }
  }
}

async fn spawn_task(task: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(task);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn encodes_and_decodes_my_message_payload() {
    let encoded = encode_prost_payload(ProstMessageKind::MyMessage, "hello".to_string());
    let decoded =
      decode_prost_payload::<mypackage::MyMessage>(ProstMessageKind::MyMessage, &encoded)
        .expect("my-message payload should decode");

    assert_eq!(decoded.content, "hello");
  }

  #[test]
  fn encodes_and_decodes_other_message_payload() {
    let encoded = encode_prost_payload(ProstMessageKind::OtherMessage, "side".to_string());
    let decoded =
      decode_prost_payload::<mypackage::OtherMessage>(ProstMessageKind::OtherMessage, &encoded)
        .expect("other-message payload should decode");

    assert_eq!(decoded.data, "side");
  }
}
