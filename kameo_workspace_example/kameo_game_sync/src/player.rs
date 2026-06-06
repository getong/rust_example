use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};

use crate::map::{EnterPlayer, MapActor, MapJoinInfo};

pub(crate) type PlayerId = u64;

// ── Value types ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerStats {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

impl PlayerStats {
  pub(crate) fn apply_damage(&mut self, red_damage: i32, blue_damage: i32) {
    self.red = (self.red - red_damage).max(0);
    self.blue = (self.blue - blue_damage).max(0);
  }
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerSnapshot {
  pub(crate) id: PlayerId,
  pub(crate) name: String,
  pub(crate) stats: PlayerStats,
}

// ── Actor ────────────────────────────────────────────────────────────────────

#[derive(Actor, RemoteActor)]
#[remote_actor(id = "kameo_game_sync::PlayerActor")]
pub(crate) struct PlayerActor {
  pub(crate) snapshot: PlayerSnapshot,
}

// ── GetPlayerSnapshot ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GetPlayerSnapshot;

#[remote_message("kameo_game_sync::PlayerActor::GetPlayerSnapshot")]
impl Message<GetPlayerSnapshot> for PlayerActor {
  type Reply = PlayerSnapshot;

  async fn handle(
    &mut self,
    _msg: GetPlayerSnapshot,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.snapshot.clone()
  }
}

// ── EnterMap ─────────────────────────────────────────────────────────────────

/// Tells the player to join a map.
///
/// `map` is a `RemoteActorRef<MapActor>` so the same message type works whether
/// the map lives in the same process or on a remote libp2p node.  In single-
/// node mode convert a local `ActorRef<MapActor>` with `.into_remote_ref()`.
pub(crate) struct EnterMap {
  pub(crate) map: RemoteActorRef<MapActor>,
}

impl Message<EnterMap> for PlayerActor {
  type Reply = MapJoinInfo;

  async fn handle(
    &mut self,
    EnterMap { map }: EnterMap,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let join_info = map
      .ask(&EnterPlayer {
        snapshot: self.snapshot.clone(),
      })
      .await
      .expect("EnterPlayer failed");

    self.snapshot = join_info.own_snapshot.clone();
    join_info
  }
}

// ── SyncFromMap ──────────────────────────────────────────────────────────────

/// Pushed from the map to the player after a stat-changing event (e.g. a hit).
/// Returns `()` — the map only needs delivery confirmation, not the full
/// snapshot back.  A network error triggers the map's 180-second eviction timer.
#[derive(Serialize, Deserialize)]
pub(crate) struct SyncFromMap {
  pub(crate) stats: PlayerStats,
}

#[remote_message("kameo_game_sync::PlayerActor::SyncFromMap")]
impl Message<SyncFromMap> for PlayerActor {
  type Reply = ();

  async fn handle(
    &mut self,
    SyncFromMap { stats }: SyncFromMap,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.snapshot.stats = stats;
    println!(
      "[player] {} synced from map: red={}, blue={}",
      self.snapshot.name, self.snapshot.stats.red, self.snapshot.stats.blue,
    );
  }
}
