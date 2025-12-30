use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use axum::{Json, Router, extract::State, routing::post};
use futures::{StreamExt, TryStreamExt};
use kameo::{
  Actor, RemoteActor,
  actor::{RemoteActorRef, Spawn},
  message::{Context as KameoContext, Message},
  remote, remote_message,
};
use libp2p::{
  PeerId, SwarmBuilder, mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::signal::ShutdownRx;

#[derive(Actor, RemoteActor)]
pub struct MyActor {
  count: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Inc {
  amount: u32,
  from: PeerId,
}

#[remote_message]
impl Message<Inc> for MyActor {
  type Reply = i64;

  async fn handle(&mut self, msg: Inc, _ctx: &mut KameoContext<Self, Self::Reply>) -> Self::Reply {
    info!(
      "<-- recv inc message from peer {}",
      &msg.from.to_base58()[46 ..]
    );
    self.count += msg.amount as i64;
    self.count
  }
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kameo: remote::Behaviour,
  mdns: mdns::tokio::Behaviour,
}

#[derive(Clone)]
struct KameoState {
  local_peer_id: PeerId,
}

#[derive(Deserialize)]
struct IncRequest {
  amount: Option<u32>,
}

#[derive(Serialize)]
struct IncResponse {
  ok: bool,
  target_peer_id: Option<String>,
  count: Option<i64>,
  error: Option<String>,
}

async fn kameo_inc(
  State(state): State<Arc<KameoState>>,
  Json(req): Json<IncRequest>,
) -> Json<IncResponse> {
  let amount = req.amount.unwrap_or(10);
  let mut incrementors = RemoteActorRef::<MyActor>::lookup_all("incrementor");
  let mut remote_incrementors = Vec::new();

  loop {
    match incrementors.try_next().await {
      Ok(Some(incrementor)) => {
        if incrementor.id().peer_id() == Some(&state.local_peer_id) {
          continue;
        }
        remote_incrementors.push(incrementor);
      }
      Ok(None) => break,
      Err(err) => {
        return Json(IncResponse {
          ok: false,
          target_peer_id: None,
          count: None,
          error: Some(format!("lookup error: {err}")),
        });
      }
    }
  }

  if remote_incrementors.is_empty() {
    return Json(IncResponse {
      ok: false,
      target_peer_id: None,
      count: None,
      error: Some("no remote incrementors available".to_string()),
    });
  }

  let index = {
    let mut rng = rand::rng();
    rng.random_range(.. remote_incrementors.len())
  };
  let incrementor = remote_incrementors.swap_remove(index);
  let target_peer_id = incrementor.id().peer_id().map(|p| p.to_string());
  let from = state.local_peer_id.clone();
  match incrementor.ask(&Inc { amount, from }).await {
    Ok(count) => Json(IncResponse {
      ok: true,
      target_peer_id,
      count: Some(count),
      error: None,
    }),
    Err(err) => Json(IncResponse {
      ok: false,
      target_peer_id,
      count: None,
      error: Some(format!("failed to increment actor: {err}")),
    }),
  }
}

async fn serve_http(
  addr: SocketAddr,
  state: Arc<KameoState>,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let app = Router::new()
    .route("/kameo/inc", post(kameo_inc))
    .with_state(state);

  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .context("bind kameo http")?;
  axum::serve(listener, app)
    .with_graceful_shutdown(async move {
      let _ = shutdown_rx.changed().await;
    })
    .await
    .context("serve kameo http")?;
  Ok(())
}

async fn bootstrap_mode() -> anyhow::Result<PeerId> {
  let local_peer_id = remote::bootstrap().map_err(|err| anyhow!("bootstrap failed: {err}"))?;
  info!("bootstrap swarm running as {}", local_peer_id.to_base58());
  Ok(local_peer_id)
}

async fn custom_swarm_mode(mut shutdown_rx: ShutdownRx) -> anyhow::Result<PeerId> {
  let mut swarm = SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_quic()
    .with_behaviour(|key| {
      let local_peer_id = key.public().to_peer_id();
      let kameo = remote::Behaviour::new(
        local_peer_id,
        remote::messaging::Config::default()
          .with_request_timeout(Duration::from_secs(30))
          .with_max_concurrent_streams(100),
      );
      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
      Ok(MyBehaviour { kameo, mdns })
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
    .build();

  swarm.behaviour().kameo.init_global();
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
  let local_peer_id = *swarm.local_peer_id();
  info!("custom swarm running as {}", local_peer_id.to_base58());

  tokio::spawn(async move {
    loop {
      tokio::select! {
        _ = shutdown_rx.changed() => {
          info!("kameo swarm shutdown signal received");
          break;
        }
        event = swarm.select_next_some() => match event {
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
          for (peer_id, multiaddr) in list {
            info!("mDNS discovered peer: {peer_id}");
            swarm.add_peer_address(peer_id, multiaddr);
          }
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
          for (peer_id, _) in list {
            warn!("mDNS peer expired: {peer_id}");
            let _ = swarm.disconnect_peer_id(peer_id);
          }
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Kameo(remote::Event::Registry(registry_event))) => {
          info!("registry event: {:?}", registry_event);
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Kameo(remote::Event::Messaging(
          messaging_event,
        ))) => {
          info!("messaging event: {:?}", messaging_event);
        }
        SwarmEvent::NewListenAddr { address, .. } => {
          info!("listening on {address}");
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
          info!("connected to {peer_id}");
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
          warn!("disconnected from {peer_id}: {cause:?}");
        }
        _ => {}
        },
      }
    }
  });

  Ok(local_peer_id)
}

pub async fn run(custom_swarm: bool, http_addr: SocketAddr) -> anyhow::Result<()> {
  let mut shutdown = crate::signal::spawn_handler();
  let swarm_shutdown = shutdown.shutdown_rx();
  let local_peer_id = if custom_swarm {
    custom_swarm_mode(swarm_shutdown).await?
  } else {
    bootstrap_mode().await?
  };

  let actor_ref = MyActor::spawn(MyActor { count: 0 });
  actor_ref.register("incrementor").await?;
  info!("registered local actor (use /kameo/inc to send messages)");

  let state = Arc::new(KameoState { local_peer_id });

  let http_done = shutdown.push("kameo-http");
  let http_shutdown = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = serve_http(http_addr, state, http_shutdown).await;
    let _ = http_done.send(res);
  });

  let _ = shutdown.wait_signal().await;
  Ok(())
}
