use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};

use crate::map::{EnterPlayer, MapActor, MapJoinInfo, MapPlayerView};

pub(crate) type PlayerId = u64;

// ── Player-owned state ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerProfile {
  pub(crate) id: PlayerId,
  pub(crate) name: String,
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerView {
  pub(crate) profile: PlayerProfile,
  pub(crate) map_mirror: Option<PlayerMapMirror>,
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerMapMirror {
  pub(crate) current_map: String,
  pub(crate) own_state: MapPlayerView,
  pub(crate) visible_players: Vec<MapPlayerView>,
}

// ── Map-owned value types mirrored by player ────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reply, Serialize, Deserialize)]
pub(crate) struct MapStats {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

impl MapStats {
  pub(crate) fn apply_damage(&mut self, damage: Damage) {
    self.red = (self.red - damage.red).max(0);
    self.blue = (self.blue - damage.blue).max(0);
  }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct InitialMapStats {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

impl From<InitialMapStats> for MapStats {
  fn from(stats: InitialMapStats) -> Self {
    Self {
      red: stats.red,
      blue: stats.blue,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Damage {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

impl Damage {
  pub(crate) fn stack(self, other: Self) -> Self {
    Self {
      red: self.red + other.red,
      blue: self.blue + other.blue,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reply, Serialize, Deserialize)]
pub(crate) struct EffectiveCombat {
  pub(crate) attack: Damage,
  pub(crate) damage: Damage,
}

impl EffectiveCombat {
  pub(crate) fn total_damage(self, bonus_damage: Damage) -> Damage {
    self.attack.stack(self.damage).stack(bonus_damage)
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PlayerBuff {
  pub(crate) name: String,
  pub(crate) attack_bonus: Damage,
  pub(crate) damage_bonus: Damage,
}

impl PlayerBuff {
  pub(crate) fn new(name: impl Into<String>, attack_bonus: Damage, damage_bonus: Damage) -> Self {
    Self {
      name: name.into(),
      attack_bonus,
      damage_bonus,
    }
  }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct InitialCombatStats {
  pub(crate) base_attack: Damage,
  pub(crate) base_damage: Damage,
}

// ── Actor ────────────────────────────────────────────────────────────────────

#[derive(Actor, RemoteActor)]
#[remote_actor(id = "kameo_game_sync::PlayerActor")]
pub(crate) struct PlayerActor {
  pub(crate) profile: PlayerProfile,
  pub(crate) initial_map_stats: InitialMapStats,
  pub(crate) initial_combat_stats: InitialCombatStats,
  pub(crate) map_mirror: Option<PlayerMapMirror>,
}

// ── GetPlayerView ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GetPlayerView;

#[remote_message("kameo_game_sync::PlayerActor::GetPlayerView")]
impl Message<GetPlayerView> for PlayerActor {
  type Reply = PlayerView;

  async fn handle(
    &mut self,
    _msg: GetPlayerView,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    PlayerView {
      profile: self.profile.clone(),
      map_mirror: self.map_mirror.clone(),
    }
  }
}

// ── EnterMap ─────────────────────────────────────────────────────────────────

/// Tells the player to join a map.
///
/// The player sends profile + initial map stats. After this point the map owns
/// map-state writes, and the player only stores the mirror received in events.
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
        profile: self.profile.clone(),
        initial_stats: self.initial_map_stats,
        initial_combat: self.initial_combat_stats,
      })
      .await
      .expect("EnterPlayer failed");

    self.map_mirror = Some(PlayerMapMirror {
      current_map: join_info.map_id.clone(),
      own_state: join_info.own_state.clone(),
      visible_players: join_info.other_players.clone(),
    });
    join_info
  }
}

// ── ApplyMapEvent ────────────────────────────────────────────────────────────

/// Pushed from the authoritative map after a state-changing event.
/// PlayerActor stores only a mirror for display/session recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum MapEvent {
  PlayerJoined {
    map_id: String,
    player: MapPlayerView,
  },
  PlayerStateChanged {
    map_id: String,
    player: MapPlayerView,
  },
  PlayerLeft {
    map_id: String,
    player_id: PlayerId,
  },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyMapEvent {
  pub(crate) event: MapEvent,
}

#[remote_message("kameo_game_sync::PlayerActor::ApplyMapEvent")]
impl Message<ApplyMapEvent> for PlayerActor {
  type Reply = ();

  async fn handle(
    &mut self,
    ApplyMapEvent { event }: ApplyMapEvent,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    match event {
      MapEvent::PlayerJoined { map_id, player } => {
        let mirror = self.map_mirror.get_or_insert_with(|| PlayerMapMirror {
          current_map: map_id,
          own_state: player.clone(),
          visible_players: Vec::new(),
        });

        upsert_visible_player(mirror, player.clone());
        println!(
          "[player] {} sees player:{} joined {}",
          self.profile.name, player.profile.id, mirror.current_map
        );
      }
      MapEvent::PlayerStateChanged { map_id, player } => {
        let mirror = self.map_mirror.get_or_insert_with(|| PlayerMapMirror {
          current_map: map_id,
          own_state: player.clone(),
          visible_players: Vec::new(),
        });

        if player.profile.id == self.profile.id {
          mirror.own_state = player.clone();
        } else {
          upsert_visible_player(mirror, player.clone());
        }

        println!(
          "[player] {} mirrored map state for player:{} => red={}, blue={}, attack=({},{}) \
           damage=({},{}) buffs={}",
          self.profile.name,
          player.profile.id,
          player.stats.red,
          player.stats.blue,
          player.effective_combat.attack.red,
          player.effective_combat.attack.blue,
          player.effective_combat.damage.red,
          player.effective_combat.damage.blue,
          player.buffs.len(),
        );
      }
      MapEvent::PlayerLeft { map_id, player_id } => {
        if let Some(mirror) = &mut self.map_mirror {
          mirror.visible_players.retain(|p| p.profile.id != player_id);
        }
        println!(
          "[player] {} sees player:{player_id} left {map_id}",
          self.profile.name
        );
      }
    }
  }
}

fn upsert_visible_player(mirror: &mut PlayerMapMirror, player: MapPlayerView) {
  if player.profile.id == mirror.own_state.profile.id {
    mirror.own_state = player;
    return;
  }

  if let Some(existing) = mirror
    .visible_players
    .iter_mut()
    .find(|p| p.profile.id == player.profile.id)
  {
    *existing = player;
  } else {
    mirror.visible_players.push(player);
  }
}
