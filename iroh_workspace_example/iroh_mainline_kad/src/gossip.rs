use std::net::{IpAddr, SocketAddr};

use iroh::{
  EndpointAddr, EndpointId, TransportAddr, address_lookup::memory::MemoryLookup, protocol::Router,
};
use iroh_gossip::{
  Gossip,
  api::{Event, GossipSender},
};
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
  util::{backoff_duration, display_values, hex_encode},
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

  println!("gossip topic: {}", options.topic);
  println!(
    "mainline target: {} (salt: {})",
    options.cluster.target(),
    hex_encode(options.cluster.salt())
  );

  let (sender, mut receiver) = match gossip.subscribe(options.topic, Vec::new()).await {
    Ok(topic) => topic.split(),
    Err(err) => {
      router.shutdown().await.std_context("shutdown router")?;
      return Err(n0_error::anyerr!(err, "failed to subscribe gossip topic"));
    }
  };

  let allow_private_addrs = options.iroh.bind.ip().is_loopback();
  let discover_task = spawn_gossip_discovery(
    dht.clone(),
    options.cluster.clone(),
    options.discover_timeout,
    router.endpoint().id(),
    address_lookup.clone(),
    sender.clone(),
    allow_private_addrs,
  );

  if !options.wait_joined.is_zero() {
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
    if receiver.neighbors().next().is_none() {
      warn!("no gossip neighbors joined yet; broadcast has no current recipients");
    }
    sender
      .broadcast(message.as_bytes().to_vec().into())
      .await
      .map_err(|err| n0_error::anyerr!(err, "failed to broadcast gossip message"))?;
    println!("gossip broadcast sent: {message}");

    if options.exit_after_broadcast {
      discover_task.abort();
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
            discover_task.abort();
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

  discover_task.abort();
  republish_task.abort();
  router.shutdown().await.std_context("shutdown router")?;
  Ok(())
}

fn spawn_gossip_discovery(
  dht: mainline::async_dht::AsyncDht,
  cluster: crate::identity::ClusterIdentity,
  discover_timeout: std::time::Duration,
  self_id: EndpointId,
  address_lookup: MemoryLookup,
  sender: GossipSender,
  allow_private_addrs: bool,
) -> tokio::task::JoinHandle<()> {
  tokio::spawn(async move {
    let mut attempt = 0;
    loop {
      match discover_members(&dht, &cluster, discover_timeout).await {
        Ok(members) => {
          let peers =
            gossip_bootstrap_peers(&members, self_id, &address_lookup, allow_private_addrs);
          if peers.is_empty() {
            attempt += 1;
          } else {
            println!("discovered {} gossip bootstrap peer(s)", peers.len());
            if let Err(err) = sender.join_peers(peers).await {
              warn!("failed to join discovered gossip peers: {err:#}");
            }
            attempt = 0;
          }
        }
        Err(err) => {
          warn!("gossip discovery attempt failed: {err:#}");
          attempt += 1;
        }
      }

      let delay =
        backoff_duration(500, attempt).min(discover_timeout.max(std::time::Duration::from_secs(5)));
      sleep(delay).await;
    }
  })
}

fn gossip_bootstrap_peers(
  members: &[MemberRecord],
  self_id: EndpointId,
  address_lookup: &MemoryLookup,
  allow_private_addrs: bool,
) -> Vec<EndpointId> {
  let mut peers = Vec::with_capacity(members.len());

  for member in members {
    if !member.supports_gossip() {
      continue;
    }

    match member.endpoint_addr() {
      Ok(addr) if addr.id == self_id => {}
      Ok(addr) => {
        let filtered = filter_endpoint_addr(addr, allow_private_addrs);
        if filtered.is_empty() {
          warn!(
            "skipping gossip member {} ({}) with no allowed transport addresses",
            member.name, filtered.id
          );
          continue;
        }
        println!("gossip bootstrap peer: {} ({})", member.name, filtered.id);
        address_lookup.add_endpoint_info(filtered.clone());
        peers.push(filtered.id);
      }
      Err(err) => {
        warn!("skipping invalid gossip member {}: {err:#}", member.name);
      }
    }
  }

  peers
}

fn filter_endpoint_addr(addr: EndpointAddr, allow_private_addrs: bool) -> EndpointAddr {
  let id = addr.id;
  let allowed = addr
    .addrs
    .into_iter()
    .filter(|transport| is_allowed_transport_addr(transport, allow_private_addrs));
  EndpointAddr::from_parts(id, allowed)
}

fn is_allowed_transport_addr(transport: &TransportAddr, allow_private_addrs: bool) -> bool {
  match transport {
    TransportAddr::Relay(_) => true,
    TransportAddr::Ip(addr) => is_allowed_socket_addr(addr, allow_private_addrs),
    TransportAddr::Custom(_) => false,
    _ => false,
  }
}

fn is_allowed_socket_addr(addr: &SocketAddr, allow_private_addrs: bool) -> bool {
  if allow_private_addrs {
    return true;
  }

  is_global_ip(addr.ip())
}

fn is_global_ip(ip: IpAddr) -> bool {
  match ip {
    IpAddr::V4(ip) => {
      !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified())
    }
    IpAddr::V6(ip) => {
      !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::str::FromStr;

  use iroh::{RelayUrl, SecretKey};

  #[test]
  fn address_filter_rejects_private_direct_addresses_by_default() {
    let id = SecretKey::from_bytes(&[1; 32]).public();
    let relay = RelayUrl::from_str("https://relay.example.com").unwrap();
    let addr = EndpointAddr::from_parts(
      id,
      [
        TransportAddr::Ip("10.1.2.3:1234".parse().unwrap()),
        TransportAddr::Ip("8.8.8.8:1234".parse().unwrap()),
        TransportAddr::Relay(relay.clone()),
      ],
    );

    let filtered = filter_endpoint_addr(addr, false);
    assert!(
      filtered
        .addrs
        .contains(&TransportAddr::Ip("8.8.8.8:1234".parse().unwrap()))
    );
    assert!(filtered.addrs.contains(&TransportAddr::Relay(relay)));
    assert!(
      !filtered
        .addrs
        .contains(&TransportAddr::Ip("10.1.2.3:1234".parse().unwrap()))
    );
  }

  #[test]
  fn address_filter_allows_private_addresses_for_local_mode() {
    let id = SecretKey::from_bytes(&[2; 32]).public();
    let addr = EndpointAddr::from_parts(
      id,
      [
        TransportAddr::Ip("127.0.0.1:1234".parse().unwrap()),
        TransportAddr::Ip("10.1.2.3:1234".parse().unwrap()),
      ],
    );

    let filtered = filter_endpoint_addr(addr, true);
    assert_eq!(filtered.addrs.len(), 2);
  }
}
