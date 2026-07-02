use iroh::{EndpointId, address_lookup::memory::MemoryLookup, protocol::Router};
use iroh_gossip::{Gossip, api::Event};
use n0_error::{Result, StdResultExt};
use n0_future::StreamExt;
use tokio::time::{sleep, timeout};
use tracing::warn;

use crate::{
  dht::{build_dht, discover_members, publish_member},
  endpoint::{build_endpoint_with_address_lookup, endpoint_ready},
  options::GossipOptions,
  protocols::GOSSIP_PROTOCOL,
  records::{MemberRecord, member_from_endpoint},
  util::display_values,
};

pub async fn run_gossip(options: GossipOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let address_lookup = MemoryLookup::with_provenance("mainline_kad");
  let endpoint =
    build_endpoint_with_address_lookup(&options.iroh, false, Some(address_lookup.clone())).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let member = member_from_endpoint(&endpoint, &options.name, &[GOSSIP_PROTOCOL]);
  println!("gossip endpoint id: {}", member.endpoint_id);
  println!("gossip direct addrs: {}", display_values(&member.addrs));
  if !member.relay_urls.is_empty() {
    println!("gossip relay urls: {}", member.relay_urls.join(" "));
  }

  let gossip = Gossip::builder().spawn(endpoint.clone());
  let router = Router::builder(endpoint)
    .accept(iroh_gossip::ALPN, gossip.clone())
    .spawn();

  if let Err(err) = publish_member(&dht, &options.cluster, member, 32).await {
    router.shutdown().await.std_context("shutdown router")?;
    return Err(err);
  }

  let discovered = match discover_members(&dht, &options.cluster, options.discover_timeout).await {
    Ok(members) => members,
    Err(err) => {
      router.shutdown().await.std_context("shutdown router")?;
      return Err(err);
    }
  };
  let bootstrap_peers =
    gossip_bootstrap_peers(&discovered, router.endpoint().id(), &address_lookup);

  println!("gossip topic: {}", options.topic);
  println!(
    "mainline target: {} (salt: {})",
    options.cluster.target(),
    String::from_utf8_lossy(options.cluster.salt())
  );
  println!(
    "discovered {} gossip bootstrap peer(s)",
    bootstrap_peers.len()
  );

  let (sender, mut receiver) = match gossip
    .subscribe(options.topic, bootstrap_peers.clone())
    .await
  {
    Ok(topic) => topic.split(),
    Err(err) => {
      router.shutdown().await.std_context("shutdown router")?;
      return Err(n0_error::anyerr!(err, "failed to subscribe gossip topic"));
    }
  };

  if !bootstrap_peers.is_empty() && !options.wait_joined.is_zero() {
    match timeout(options.wait_joined, receiver.joined()).await {
      Ok(Ok(())) => {
        let neighbors = receiver
          .neighbors()
          .map(|peer| peer.to_string())
          .collect::<Vec<_>>();
        println!("gossip joined peer(s): {}", display_values(&neighbors));
      }
      Ok(Err(err)) => {
        router.shutdown().await.std_context("shutdown router")?;
        return Err(n0_error::anyerr!(err, "failed to join gossip topic"));
      }
      Err(_) => {
        warn!(
          "timed out waiting {:?} for a gossip neighbor; continuing",
          options.wait_joined
        );
      }
    }
  }

  if let Some(message) = options.message.as_deref() {
    if bootstrap_peers.is_empty() {
      warn!("no gossip bootstrap peers discovered; broadcast has no current recipients");
    }
    sender
      .broadcast(message.as_bytes().to_vec().into())
      .await
      .map_err(|err| n0_error::anyerr!(err, "failed to broadcast gossip message"))?;
    println!("gossip broadcast sent: {message}");

    if options.exit_after_broadcast {
      router.shutdown().await.std_context("shutdown router")?;
      return Ok(());
    }
  }

  let dht_for_publish = dht.clone();
  let cluster_for_publish = options.cluster.clone();
  let endpoint_for_publish = router.endpoint().clone();
  let name = options.name.clone();
  let republish_every = options.republish_every;
  let republish_task = tokio::spawn(async move {
    loop {
      sleep(republish_every).await;
      let member = member_from_endpoint(&endpoint_for_publish, &name, &[GOSSIP_PROTOCOL]);
      if let Err(err) = publish_member(&dht_for_publish, &cluster_for_publish, member, 32).await {
        warn!("failed to republish gossip member: {err:#}");
      }
    }
  });

  println!("gossip peer is running. press ctrl-c to stop.");
  loop {
    tokio::select! {
      _ = tokio::signal::ctrl_c() => {
        break;
      }
      event = receiver.next() => {
        match event {
          Some(Ok(Event::Received(message))) => {
            let content = String::from_utf8_lossy(&message.content);
            println!("gossip received from {}: {content}", message.delivered_from);
          }
          Some(Ok(Event::NeighborUp(peer))) => {
            println!("gossip neighbor up: {peer}");
          }
          Some(Ok(Event::NeighborDown(peer))) => {
            println!("gossip neighbor down: {peer}");
          }
          Some(Ok(Event::Lagged)) => {
            warn!("gossip receiver lagged and missed messages");
          }
          Some(Err(err)) => {
            republish_task.abort();
            router.shutdown().await.std_context("shutdown router")?;
            return Err(n0_error::anyerr!(err, "gossip receiver failed"));
          }
          None => {
            break;
          }
        }
      }
    }
  }

  republish_task.abort();
  router.shutdown().await.std_context("shutdown router")?;
  Ok(())
}

fn gossip_bootstrap_peers(
  members: &[MemberRecord],
  self_id: EndpointId,
  address_lookup: &MemoryLookup,
) -> Vec<EndpointId> {
  let mut peers = Vec::new();

  for member in members {
    if !member.supports_gossip() {
      continue;
    }

    match member.endpoint_addr() {
      Ok(addr) if addr.id == self_id => {}
      Ok(addr) => {
        println!("gossip bootstrap peer: {} ({})", member.name, addr.id);
        address_lookup.add_endpoint_info(addr.clone());
        peers.push(addr.id);
      }
      Err(err) => {
        warn!("skipping invalid gossip member {}: {err:#}", member.name);
      }
    }
  }

  peers
}
