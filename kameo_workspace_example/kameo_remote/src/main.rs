use std::{env, time::Duration};

use futures::{StreamExt, TryStreamExt};
use kameo::prelude::*;
use libp2p::{
  PeerId, SwarmBuilder, mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

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

  async fn handle(&mut self, msg: Inc, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
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

async fn register_and_run(local_peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
  let actor_ref = MyActor::spawn(MyActor { count: 0 });
  actor_ref.register("incrementor").await?;
  info!("registered local actor");

  loop {
    let mut incrementors = RemoteActorRef::<MyActor>::lookup_all("incrementor");
    while let Some(incrementor) = incrementors.try_next().await? {
      if incrementor.id().peer_id() == Some(&local_peer_id) {
        continue;
      }

      match incrementor
        .ask(&Inc {
          amount: 10,
          from: local_peer_id,
        })
        .await
      {
        Ok(count) => info!("--> send inc: count is {count}"),
        Err(err) => error!("failed to increment actor: {err}"),
      }
    }

    tokio::time::sleep(Duration::from_secs(3)).await;
  }
}

async fn bootstrap_mode() -> Result<(), Box<dyn std::error::Error>> {
  let local_peer_id = remote::bootstrap()?;
  info!("bootstrap swarm running as {}", local_peer_id.to_base58());
  register_and_run(local_peer_id).await
}

async fn custom_swarm_mode() -> Result<(), Box<dyn std::error::Error>> {
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
      match swarm.select_next_some().await {
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
      }
    }
  });

  register_and_run(local_peer_id).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .with_env_filter("info".parse::<EnvFilter>()?)
    .without_time()
    .with_target(false)
    .init();

  if env::var("CUSTOM_SWARM").is_ok() {
    custom_swarm_mode().await
  } else {
    bootstrap_mode().await
  }
}
