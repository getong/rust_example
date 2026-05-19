use std::{future::Future, net::SocketAddr, time::Duration};

use anyhow::{Context as _, anyhow};
use clap::{Parser, Subcommand};
use futures::{StreamExt, future};
use kameo::{prelude::*, remote};
use libp2p::{
  Multiaddr, PeerId, SwarmBuilder,
  multiaddr::Protocol,
  noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tarpc::{
  client, context,
  server::{self, Channel},
  tokio_serde::formats::Json,
};
use tokio::time::sleep;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

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
    #[arg(long)]
    amount: u32,
    #[arg(long, default_value = "demo-client")]
    caller: String,
  },
}

#[derive(Parser, Debug, Clone)]
struct NodeArgs {
  #[arg(long, default_value = DEFAULT_ACTOR_NAME)]
  actor_name: String,
  #[arg(long, default_value = "/ip4/127.0.0.1/tcp/4101")]
  swarm_listen_addr: Multiaddr,
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

#[derive(Actor, RemoteActor)]
struct CounterActor {
  total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Add {
  amount: u32,
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

#[remote_message]
impl Message<Add> for CounterActor {
  type Reply = CounterSnapshot;

  async fn handle(&mut self, msg: Add, ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    let previous_total = self.total;
    self.total += i64::from(msg.amount);
    let new_total = self.total;
    info!(
      "counter actor handled caller={} amount={} previous_total={} new_total={} actor_id={}",
      msg.caller,
      msg.amount,
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
      acknowledged_caller: msg.caller,
    }
  }
}

#[tarpc::service]
trait CounterRpc {
  async fn add(amount: u32, caller: String) -> Result<RpcAddResponse, RpcError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcAddResponse {
  actor_registration: String,
  actor_id: String,
  actor_peer: String,
  rpc_server_peer: String,
  total: i64,
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

impl CounterRpc for CounterRpcServer {
  async fn add(
    self,
    _: context::Context,
    amount: u32,
    caller: String,
  ) -> Result<RpcAddResponse, RpcError> {
    let snapshot = forward_to_counter(&self.actor_name, self.local_peer_id, amount, caller.clone())
      .await
      .map_err(|err| RpcError::RemoteActorUnavailable {
        message: err.to_string(),
      })?;

    Ok(RpcAddResponse {
      actor_registration: self.actor_name,
      actor_id: snapshot.actor_id,
      actor_peer: snapshot.handled_by_peer,
      rpc_server_peer: self.local_peer_id.to_string(),
      total: snapshot.total,
      caller: snapshot.acknowledged_caller,
    })
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
      amount,
      caller,
    } => run_rpc_client(server_addr, amount, caller).await,
  }
}

async fn run_actor_node(args: NodeArgs) -> anyhow::Result<()> {
  let local_peer_id = start_swarm(&args.swarm_listen_addr, &args.seed_addrs).await?;
  info!(
    "actor node ready peer_id={local_peer_id} actor_name={} listen_addr={}",
    args.actor_name, args.swarm_listen_addr
  );

  let actor_ref = CounterActor::spawn(CounterActor { total: 0 });
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

  let reply = client
    .add(context::current(), amount, caller.clone())
    .await
    .context("tarpc add rpc failed")?
    .map_err(|err| anyhow!("rpc server returned error: {err:?}"))?;

  println!(
    "rpc_client caller={} amount={} total={} actor_registration={} actor_id={} actor_peer={} \
     rpc_server_peer={}",
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
  amount: u32,
  caller: String,
) -> anyhow::Result<CounterSnapshot> {
  let remote_actor = lookup_counter(actor_name, local_peer_id).await?;
  let target_actor_id = remote_actor.id();
  info!("forwarding rpc to actor_name={actor_name} actor_id={target_actor_id}");

  remote_actor
    .ask(&Add {
      amount,
      caller,
      origin_peer: local_peer_id,
    })
    .send()
    .await
    .map_err(|err| anyhow!("remote actor call failed: {err}"))
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

async fn spawn_task(task: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(task);
}
