//! Gossipsub event handling: node announcements, chat messages, and the
//! openraft snapshot partial sync protocol.

use std::collections::HashMap;

use libp2p::{Swarm, gossipsub};
use prost::Message;
use tokio::sync::mpsc;

use crate::{
  network::{
    openraft_sync::{
      OpenRaftSyncState, SnapshotAvailableAnnouncement, available_topic_hash, group_id_string,
      sync_topic_hash,
    },
    swarm::{Behaviour, NODE_ANNOUNCE_TOPIC},
    transport::Libp2pNetworkFactory,
  },
  proto::raft_kv::{ChatMessage, NodeAnnouncement},
};

/// Bounded queue between the swarm loop and the node-announce processor
/// task. Announcements are periodic and idempotent, so dropping the overflow
/// under a burst is safe — the next announce round re-delivers.
pub(crate) const NODE_ANNOUNCE_QUEUE: usize = 1024;
/// Max announcements the processor coalesces per round; duplicates from the
/// same node are folded into the newest one before touching address-book
/// locks.
const NODE_ANNOUNCE_BATCH: usize = 256;

pub(crate) fn node_announce_topic_hash() -> gossipsub::TopicHash {
  gossipsub::IdentTopic::new(NODE_ANNOUNCE_TOPIC).hash()
}

/// Process node self-announcements queued by the swarm loop: (re)register
/// each sender in the local known-nodes address book and refresh its
/// liveness timestamp. This is what brings a node back into `known_nodes`
/// after it crashed, was pruned, and restarted — mdns re-discovery alone can
/// lag by minutes.
///
/// Announcements are drained in batches and deduplicated by node id (keeping
/// the newest) before touching the address-book locks, so a gossip burst —
/// e.g. right after a network partition heals — costs O(distinct nodes)
/// lock acquisitions, not O(messages). Ends when the swarm loop drops its
/// sender.
///
/// Registration only: announcements never trigger a dial. Every node hears
/// every announcement, so dialing here would have each node open connections
/// to the whole cluster (O(N^2) connections in total).
pub(crate) async fn run_node_announce_processor(
  network: Libp2pNetworkFactory,
  mut announce_rx: mpsc::Receiver<Vec<u8>>,
) {
  let mut raw = Vec::with_capacity(NODE_ANNOUNCE_BATCH);
  let mut latest: HashMap<String, NodeAnnouncement> = HashMap::new();
  loop {
    if announce_rx.recv_many(&mut raw, NODE_ANNOUNCE_BATCH).await == 0 {
      return;
    }
    for data in raw.drain(..) {
      match NodeAnnouncement::decode(data.as_slice()) {
        Ok(announcement) => {
          latest.insert(announcement.node_id.clone(), announcement);
        }
        Err(err) => {
          tracing::debug!(error = %err, "invalid node announcement message");
        }
      }
    }
    for (_, announcement) in latest.drain() {
      if let Err(err) = network
        .register_announced_node(
          crate::NodeId::new(&announcement.node_id),
          &announcement.addr,
          announcement.announce_interval_ms,
        )
        .await
      {
        tracing::debug!(
          node_id = %announcement.node_id,
          addr = %announcement.addr,
          error = ?err,
          "failed to register announced node"
        );
      }
    }
  }
}

/// React to a "snapshot available" announcement: when the referenced
/// snapshot is unknown or incomplete locally, publish our (possibly empty)
/// partial on the sync topic. That advertises our need bitmap to mesh peers,
/// which respond by pushing the parts we lack — the pull half of the
/// otherwise push-only partial sync.
fn handle_snapshot_available(
  swarm: &mut Swarm<Behaviour>,
  openraft_sync: &mut OpenRaftSyncState,
  peer_id: libp2p::PeerId,
  data: &[u8],
) {
  let announcement = match SnapshotAvailableAnnouncement::decode(data) {
    Ok(announcement) => announcement,
    Err(err) => {
      tracing::debug!(peer = %peer_id, error = ?err, "invalid snapshot-available announcement");
      return;
    }
  };

  match openraft_sync.handle_available_announcement(&announcement) {
    Ok(Some(partial)) => {
      tracing::debug!(
        peer = %peer_id,
        group = %partial.raft_group_id,
        snapshot_id = %partial.snapshot_id,
        parts = partial.present_parts(),
        total_parts = partial.total_parts(),
        "snapshot-available announcement for missing snapshot; requesting parts"
      );
      if let Err(err) = swarm
        .behaviour_mut()
        .gossipsub
        .publish_partial(sync_topic_hash(), partial)
      {
        tracing::debug!(error = ?err, "advertise need bitmap for announced snapshot failed");
      }
    }
    Ok(None) => {}
    Err(err) => {
      tracing::debug!(
        peer = %peer_id,
        error = ?err,
        "snapshot-available announcement rejected"
      );
    }
  }
}

pub(crate) async fn handle_gossipsub_event(
  swarm: &mut Swarm<Behaviour>,
  announce_tx: &mpsc::Sender<Vec<u8>>,
  openraft_sync: &mut OpenRaftSyncState,
  event: gossipsub::Event,
) {
  match event {
    gossipsub::Event::Message {
      propagation_source,
      message_id,
      message,
    } => {
      if message.topic == node_announce_topic_hash() {
        // Announce volume grows with cluster size (every node, every
        // announce interval) and registration takes address-book locks, so
        // hand it to the dedicated processor task: the swarm loop pays one
        // try_send per announcement, and a burst must not delay raft RPC
        // event handling. Overflow is dropped — announcements are periodic
        // and idempotent, the next round re-delivers.
        if announce_tx.try_send(message.data).is_err() {
          tracing::debug!(
            peer = %propagation_source,
            "node announce queue full; dropping announcement"
          );
        }
        return;
      }

      if message.topic == available_topic_hash() {
        handle_snapshot_available(swarm, openraft_sync, propagation_source, &message.data);
        return;
      }

      match ChatMessage::decode(message.data.as_slice()) {
        Ok(chat) => {
          tracing::info!(
            peer = %propagation_source,
            message_id = %message_id,
            from = %chat.from,
            text = %chat.text,
            ts = chat.ts_unix_ms,
            "chat message"
          );
        }
        Err(err) => {
          tracing::info!(
            peer = %propagation_source,
            message_id = %message_id,
            len = message.data.len(),
            error = %err,
            "gossipsub message (decode failed)"
          );
        }
      }
    }
    gossipsub::Event::Partial {
      topic_hash,
      peer_id,
      group_id,
      message,
      metadata,
    } => {
      if topic_hash != sync_topic_hash() {
        tracing::debug!(peer = %peer_id, topic = %topic_hash, "partial message on unknown topic");
        return;
      }

      let Some(metadata) = metadata else {
        tracing::warn!(peer = %peer_id, group = %group_id_string(&group_id), "openraft snapshot partial missing metadata");
        swarm
          .behaviour_mut()
          .gossipsub
          .report_invalid_partial(peer_id, &topic_hash);
        return;
      };

      let update =
        match openraft_sync.handle_partial(group_id.clone(), &metadata, message.as_deref()) {
          Ok(update) => update,
          Err(err) => {
            tracing::warn!(
              peer = %peer_id,
              group = %group_id_string(&group_id),
              error = ?err,
              "invalid openraft snapshot partial"
            );
            swarm
              .behaviour_mut()
              .gossipsub
              .report_invalid_partial(peer_id, &topic_hash);
            return;
          }
        };

      if update.should_republish {
        if let Err(err) = swarm
          .behaviour_mut()
          .gossipsub
          .publish_partial(topic_hash.clone(), update.partial.clone())
        {
          tracing::debug!(error = ?err, "republish openraft snapshot partial failed");
        }
      }

      if update.first_complete {
        // Installing a snapshot feeds it through the raft state machine and
        // can take seconds; run it off-loop so swarm event processing (raft
        // RPCs included) is not stalled behind it.
        let partial = update.partial;
        tokio::spawn(async move {
          let raft_group_id = partial.raft_group_id.clone();
          let snapshot_id = partial.snapshot_id.clone();
          match partial.install().await {
            Ok(resp) => {
              tracing::info!(
                peer = %peer_id,
                group = %raft_group_id,
                snapshot_id = %snapshot_id,
                response = ?resp,
                "installed openraft snapshot from gossipsub partial sync"
              );
            }
            Err(err) => {
              tracing::warn!(
                peer = %peer_id,
                group = %raft_group_id,
                snapshot_id = %snapshot_id,
                error = ?err,
                "failed to install openraft snapshot from gossipsub partial sync"
              );
            }
          }
        });
      } else {
        tracing::debug!(
          peer = %peer_id,
          group = %update.partial.raft_group_id,
          snapshot_id = %update.partial.snapshot_id,
          partial_group = %group_id_string(&update.partial.group_id),
          parts = update.partial.present_parts(),
          total_parts = update.partial.total_parts(),
          "received openraft snapshot partial"
        );
      }
    }
    gossipsub::Event::Subscribed {
      peer_id,
      topic,
      supports_partial,
      ..
    } => {
      if topic != sync_topic_hash() || !supports_partial {
        tracing::debug!("gossipsub subscribed: peer={peer_id} topic={topic}");
        return;
      }

      let known_snapshots = openraft_sync.known_partials();
      for partial in known_snapshots {
        if let Err(err) = swarm
          .behaviour_mut()
          .gossipsub
          .publish_partial(topic.clone(), partial)
        {
          tracing::debug!(
            peer = %peer_id,
            error = ?err,
            "advertise known openraft snapshot partial failed"
          );
        }
      }
    }
    other => {
      tracing::debug!("gossipsub event: {:?}", other);
    }
  }
}
