//! Map actor — authoritative source of truth for all player stats.
//!
//! Cross-node optimizations:
//! • `EnterPlayer` carries no `ActorRef`; the map resolves the player via
//!   `RemoteActorRef::lookup("player:{id}")`, making the message serialisable
//!   so it can travel over a libp2p TCP link.
//! • Stat sync is fire-and-forget (`tell`): map replies to the caller first,
//!   then a background task does the remote lookup + push.
//! • 180-second grace period: if sync fails, `PlayerDisconnected` starts an
//!   `AbortHandle`-backed eviction timer.  Reconnecting within 180 s cancels
//!   the timer and restores the server-side snapshot.

use std::{collections::HashMap, time::Duration};

use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::task::AbortHandle;

use crate::player::{PlayerActor, PlayerId, PlayerSnapshot, PlayerStats, SyncFromMap};

// ── Actor ────────────────────────────────────────────────────────────────────

#[derive(Actor, Default, RemoteActor)]
#[remote_actor(id = "kameo_game_sync::MapActor")]
pub(crate) struct MapActor {
  players: HashMap<PlayerId, PlayerInMap>,
}

// ── Internal state ───────────────────────────────────────────────────────────

enum PlayerStatus {
  Online,
  /// TCP dropped.  `abort_handle` cancels the eviction timer on reconnect.
  Offline {
    abort_handle: AbortHandle,
  },
}

struct PlayerInMap {
  /// Swarm name used for remote-ref lookup: `"player:{id}"`.
  player_name: String,
  /// Server-authoritative snapshot — survives disconnects.
  snapshot: PlayerSnapshot,
  status: PlayerStatus,
}

// ── EnterPlayer ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct EnterPlayer {
  pub(crate) snapshot: PlayerSnapshot,
}

/// Returned on join: own snapshot + every other player already in the map.
/// One round-trip gives the client everything it needs to render the room.
#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct MapJoinInfo {
  pub(crate) own_snapshot: PlayerSnapshot,
  pub(crate) other_players: Vec<PlayerSnapshot>,
}

#[remote_message("kameo_game_sync::MapActor::EnterPlayer")]
impl Message<EnterPlayer> for MapActor {
  type Reply = MapJoinInfo;

  async fn handle(
    &mut self,
    EnterPlayer { snapshot }: EnterPlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let other_players: Vec<PlayerSnapshot> =
      self.players.values().map(|p| p.snapshot.clone()).collect();

    let player_name = format!("player:{}", snapshot.id);

    // ── Reconnect path: cancel timer, restore server snapshot ────────────
    if let Some(existing) = self.players.get_mut(&snapshot.id) {
      if let PlayerStatus::Offline { abort_handle } = &existing.status {
        abort_handle.abort();
        let preserved = existing.snapshot.clone();
        existing.status = PlayerStatus::Online;
        println!(
          "[map] {} reconnected within grace period — state restored (red={}, blue={})",
          existing.snapshot.name, preserved.stats.red, preserved.stats.blue,
        );
        return MapJoinInfo {
          own_snapshot: preserved,
          other_players,
        };
      }
    }

    // ── Fresh join ───────────────────────────────────────────────────────
    println!(
      "[map] {} enters: red={}, blue={} | {} other(s) present",
      snapshot.name,
      snapshot.stats.red,
      snapshot.stats.blue,
      other_players.len(),
    );
    self.players.insert(
      snapshot.id,
      PlayerInMap {
        player_name,
        snapshot: snapshot.clone(),
        status: PlayerStatus::Online,
      },
    );
    MapJoinInfo {
      own_snapshot: snapshot,
      other_players,
    }
  }
}

// ── HitPlayer ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Damage {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct HitPlayer {
  pub(crate) player_id: PlayerId,
  pub(crate) damage: Damage,
}

#[derive(Debug, Reply, Serialize, Deserialize)]
pub(crate) struct HitReport {
  pub(crate) player_id: PlayerId,
  pub(crate) stats_after_hit: PlayerStats,
}

#[remote_message("kameo_game_sync::MapActor::HitPlayer")]
impl Message<HitPlayer> for MapActor {
  type Reply = Option<HitReport>;

  async fn handle(
    &mut self,
    HitPlayer { player_id, damage }: HitPlayer,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let player = self.players.get_mut(&player_id)?;

    player.snapshot.stats.apply_damage(damage.red, damage.blue);
    let stats = player.snapshot.stats;
    let player_name = player.player_name.clone();
    println!(
      "[map] {} hit: -{} red, -{} blue => red={}, blue={}",
      player.snapshot.name, damage.red, damage.blue, stats.red, stats.blue,
    );

    // Fire-and-forget: reply first, then push the stat update to the player.
    // Use `ask` (not `tell`) so we get a Result back — a network error means
    // the player disconnected and we start the 180-second eviction timer.
    let map_ref = ctx.actor_ref().clone();
    tokio::spawn(async move {
      match RemoteActorRef::<PlayerActor>::lookup(player_name.as_str()).await {
        Ok(Some(remote_ref)) => {
          if let Err(e) = remote_ref.ask(&SyncFromMap { stats }).await {
            println!("[map] sync failed for {player_name}: {e}");
            let _ = map_ref.tell(PlayerDisconnected { player_id }).await;
          }
        }
        Ok(None) => {
          println!("[map] {player_name} not in registry — marking offline");
          let _ = map_ref.tell(PlayerDisconnected { player_id }).await;
        }
        Err(e) => println!("[map] lookup error for {player_name}: {e}"),
      }
    });

    Some(HitReport {
      player_id,
      stats_after_hit: stats,
    })
  }
}

// ── PlayerDisconnected ───────────────────────────────────────────────────────

/// Sent internally when a background sync task cannot reach the player.
/// Starts the 180-second eviction countdown.
pub(crate) struct PlayerDisconnected {
  pub(crate) player_id: PlayerId,
}

impl Message<PlayerDisconnected> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    PlayerDisconnected { player_id }: PlayerDisconnected,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let Some(player) = self.players.get_mut(&player_id) else {
      return;
    };

    if matches!(player.status, PlayerStatus::Online) {
      println!(
        "[map] {} disconnected — grace period: 180 s",
        player.snapshot.name
      );

      // WeakActorRef prevents the timer from keeping the map alive.
      let map_weak = ctx.actor_ref().downgrade();
      let abort_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(180)).await;
        if let Some(map) = map_weak.upgrade() {
          let _ = map.tell(RemovePlayer { player_id }).await;
        }
      })
      .abort_handle();

      player.status = PlayerStatus::Offline { abort_handle };
    }
  }
}

// ── RemovePlayer ─────────────────────────────────────────────────────────────

/// Sent by the eviction timer after the 180-second grace period expires.
pub(crate) struct RemovePlayer {
  pub(crate) player_id: PlayerId,
}

impl Message<RemovePlayer> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    RemovePlayer { player_id }: RemovePlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    if let Some(player) = self.players.remove(&player_id) {
      println!(
        "[map] {} evicted after 180 s disconnect timeout",
        player.snapshot.name
      );
    }
  }
}

// ── Queries ──────────────────────────────────────────────────────────────────

/// Full room snapshot — useful for a client joining late or after a reconnect.
#[derive(Serialize, Deserialize)]
pub(crate) struct GetAllPlayers;

#[remote_message("kameo_game_sync::MapActor::GetAllPlayers")]
impl Message<GetAllPlayers> for MapActor {
  type Reply = Vec<PlayerSnapshot>;

  async fn handle(
    &mut self,
    _msg: GetAllPlayers,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.players.values().map(|p| p.snapshot.clone()).collect()
  }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct GetMapPlayer {
  pub(crate) player_id: PlayerId,
}

#[remote_message("kameo_game_sync::MapActor::GetMapPlayer")]
impl Message<GetMapPlayer> for MapActor {
  type Reply = Option<PlayerSnapshot>;

  async fn handle(
    &mut self,
    GetMapPlayer { player_id }: GetMapPlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.players.get(&player_id).map(|p| p.snapshot.clone())
  }
}
