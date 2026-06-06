//! Map actor — authoritative source of truth for map-local combat state.
//!
//! Player actors send commands (join, hit in this demo). The map validates and
//! mutates map-owned state, then emits events back to player actors. Player
//! actors keep only a mirror for display/session purposes.

use std::{collections::HashMap, time::Duration};

use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::task::AbortHandle;

use crate::player::{
  ApplyMapEvent, InitialMapStats, MapEvent, MapStats, PlayerActor, PlayerId, PlayerProfile,
};

const MAP_ID: &str = "green-fields";

// ── Actor ────────────────────────────────────────────────────────────────────

#[derive(Actor, Default, RemoteActor)]
#[remote_actor(id = "kameo_game_sync::MapActor")]
pub(crate) struct MapActor {
  players: HashMap<PlayerId, PlayerInMap>,
}

// ── Authoritative map state ──────────────────────────────────────────────────

enum PlayerStatus {
  Online,
  Offline { abort_handle: AbortHandle },
}

struct PlayerInMap {
  player_name: String,
  state: MapPlayerState,
  status: PlayerStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MapPlayerState {
  profile: PlayerProfile,
  stats: MapStats,
}

impl MapPlayerState {
  fn view(&self) -> MapPlayerView {
    MapPlayerView {
      profile: self.profile.clone(),
      stats: self.stats,
    }
  }
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct MapPlayerView {
  pub(crate) profile: PlayerProfile,
  pub(crate) stats: MapStats,
}

// ── EnterPlayer ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct EnterPlayer {
  pub(crate) profile: PlayerProfile,
  pub(crate) initial_stats: InitialMapStats,
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct MapJoinInfo {
  pub(crate) map_id: String,
  pub(crate) own_state: MapPlayerView,
  pub(crate) other_players: Vec<MapPlayerView>,
}

#[remote_message("kameo_game_sync::MapActor::EnterPlayer")]
impl Message<EnterPlayer> for MapActor {
  type Reply = MapJoinInfo;

  async fn handle(
    &mut self,
    EnterPlayer {
      profile,
      initial_stats,
    }: EnterPlayer,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let other_players: Vec<MapPlayerView> = self
      .players
      .values()
      .filter(|p| p.state.profile.id != profile.id)
      .map(|p| p.state.view())
      .collect();
    let player_name = format!("player:{}", profile.id);

    if let Some(existing) = self.players.get_mut(&profile.id) {
      if let PlayerStatus::Offline { abort_handle } = &existing.status {
        abort_handle.abort();
        existing.status = PlayerStatus::Online;
      }

      let own_state = existing.state.view();
      println!(
        "[map] {} reconnected; authoritative state restored (red={}, blue={})",
        own_state.profile.name, own_state.stats.red, own_state.stats.blue,
      );
      return MapJoinInfo {
        map_id: MAP_ID.to_string(),
        own_state,
        other_players,
      };
    }

    let state = MapPlayerState {
      profile: profile.clone(),
      stats: initial_stats.into(),
    };
    let own_state = state.view();

    println!(
      "[map] {} enters: red={}, blue={} | {} other(s) present",
      own_state.profile.name,
      own_state.stats.red,
      own_state.stats.blue,
      other_players.len(),
    );
    self.players.insert(
      profile.id,
      PlayerInMap {
        player_name,
        state,
        status: PlayerStatus::Online,
      },
    );

    self.broadcast_event(
      ctx.actor_ref(),
      profile.id,
      MapEvent::PlayerJoined {
        map_id: MAP_ID.to_string(),
        player: own_state.clone(),
      },
    );

    MapJoinInfo {
      map_id: MAP_ID.to_string(),
      own_state,
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
  pub(crate) state_after_hit: MapPlayerView,
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

    player.state.stats.apply_damage(damage.red, damage.blue);
    let state_after_hit = player.state.view();
    println!(
      "[map] {} hit: -{} red, -{} blue => red={}, blue={}",
      state_after_hit.profile.name,
      damage.red,
      damage.blue,
      state_after_hit.stats.red,
      state_after_hit.stats.blue,
    );

    self.broadcast_event(
      ctx.actor_ref(),
      player_id,
      MapEvent::PlayerStatsChanged {
        map_id: MAP_ID.to_string(),
        player: state_after_hit.clone(),
      },
    );

    Some(HitReport {
      player_id,
      state_after_hit,
    })
  }
}

// ── PlayerDisconnected ───────────────────────────────────────────────────────

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
        player.state.profile.name
      );

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

pub(crate) struct RemovePlayer {
  pub(crate) player_id: PlayerId,
}

impl Message<RemovePlayer> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    RemovePlayer { player_id }: RemovePlayer,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    if let Some(player) = self.players.remove(&player_id) {
      println!(
        "[map] {} evicted after 180 s disconnect timeout",
        player.state.profile.name
      );

      self.broadcast_event(
        ctx.actor_ref(),
        player_id,
        MapEvent::PlayerLeft {
          map_id: MAP_ID.to_string(),
          player_id,
        },
      );
    }
  }
}

// ── Queries ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GetAllPlayers;

#[remote_message("kameo_game_sync::MapActor::GetAllPlayers")]
impl Message<GetAllPlayers> for MapActor {
  type Reply = Vec<MapPlayerView>;

  async fn handle(
    &mut self,
    _msg: GetAllPlayers,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.players.values().map(|p| p.state.view()).collect()
  }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct GetMapPlayer {
  pub(crate) player_id: PlayerId,
}

#[remote_message("kameo_game_sync::MapActor::GetMapPlayer")]
impl Message<GetMapPlayer> for MapActor {
  type Reply = Option<MapPlayerView>;

  async fn handle(
    &mut self,
    GetMapPlayer { player_id }: GetMapPlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.players.get(&player_id).map(|p| p.state.view())
  }
}

impl MapActor {
  fn broadcast_event(&self, map_ref: &ActorRef<Self>, source_player_id: PlayerId, event: MapEvent) {
    for player in self.players.values() {
      let player_id = player.state.profile.id;
      if matches!(event, MapEvent::PlayerJoined { .. }) && player_id == source_player_id {
        continue;
      }
      let player_name = player.player_name.clone();
      let event = event.clone();
      let map_ref = map_ref.clone();

      tokio::spawn(async move {
        match RemoteActorRef::<PlayerActor>::lookup(player_name.as_str()).await {
          Ok(Some(remote_ref)) => {
            if let Err(e) = remote_ref.ask(&ApplyMapEvent { event }).await {
              println!("[map] event push failed for {player_name}: {e}");
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
    }
  }
}
