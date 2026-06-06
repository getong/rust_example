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
  ApplyMapEvent, Damage, EffectiveCombat, InitialCombatStats, InitialMapStats, MapEvent, MapStats,
  PlayerActor, PlayerBuff, PlayerId, PlayerProfile,
};

// ── Actor ────────────────────────────────────────────────────────────────────

#[derive(Actor, RemoteActor)]
#[remote_actor(id = "kameo_game_sync::MapActor")]
pub(crate) struct MapActor {
  map_id: String,
  entry_buffs: Vec<PlayerBuff>,
  players: HashMap<PlayerId, PlayerInMap>,
}

#[derive(Clone, Debug)]
pub(crate) struct MapConfig {
  pub(crate) id: String,
  pub(crate) entry_buffs: Vec<PlayerBuff>,
}

impl MapConfig {
  pub(crate) fn new(id: impl Into<String>, entry_buffs: Vec<PlayerBuff>) -> Self {
    Self {
      id: id.into(),
      entry_buffs,
    }
  }
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
  base_attack: Damage,
  base_damage: Damage,
  buffs: Vec<PlayerBuff>,
}

impl MapPlayerState {
  fn view(&self) -> MapPlayerView {
    MapPlayerView {
      profile: self.profile.clone(),
      stats: self.stats,
      base_attack: self.base_attack,
      base_damage: self.base_damage,
      effective_combat: self.effective_combat(),
      buffs: self.buffs.clone(),
    }
  }

  fn add_buff(&mut self, buff: PlayerBuff) {
    self.buffs.push(buff);
  }

  fn effective_combat(&self) -> EffectiveCombat {
    let mut attack = self.base_attack;
    let mut damage = self.base_damage;

    for buff in &self.buffs {
      attack = attack.stack(buff.attack_bonus);
      damage = damage.stack(buff.damage_bonus);
    }

    EffectiveCombat { attack, damage }
  }
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct MapPlayerView {
  pub(crate) profile: PlayerProfile,
  pub(crate) stats: MapStats,
  pub(crate) base_attack: Damage,
  pub(crate) base_damage: Damage,
  pub(crate) effective_combat: EffectiveCombat,
  pub(crate) buffs: Vec<PlayerBuff>,
}

// ── EnterPlayer ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct EnterPlayer {
  pub(crate) profile: PlayerProfile,
  pub(crate) initial_stats: InitialMapStats,
  pub(crate) initial_combat: InitialCombatStats,
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
      initial_combat,
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
        "[map:{}] {} reconnected; authoritative state restored (red={}, blue={}, buffs={})",
        self.map_id,
        own_state.profile.name,
        own_state.stats.red,
        own_state.stats.blue,
        own_state.buffs.len(),
      );
      return MapJoinInfo {
        map_id: self.map_id.clone(),
        own_state,
        other_players,
      };
    }

    let state = MapPlayerState {
      profile: profile.clone(),
      stats: initial_stats.into(),
      base_attack: initial_combat.base_attack,
      base_damage: initial_combat.base_damage,
      buffs: self.entry_buffs.clone(),
    };
    let own_state = state.view();

    println!(
      "[map:{}] {} enters: red={}, blue={}, attack=({},{}) damage=({},{}) buffs={} | {} other(s) \
       present",
      self.map_id,
      own_state.profile.name,
      own_state.stats.red,
      own_state.stats.blue,
      own_state.effective_combat.attack.red,
      own_state.effective_combat.attack.blue,
      own_state.effective_combat.damage.red,
      own_state.effective_combat.damage.blue,
      own_state.buffs.len(),
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
        map_id: self.map_id.clone(),
        player: own_state.clone(),
      },
    );

    MapJoinInfo {
      map_id: self.map_id.clone(),
      own_state,
      other_players,
    }
  }
}

// ── Buffs ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GivePlayerBuff {
  pub(crate) player_id: PlayerId,
  pub(crate) buff: PlayerBuff,
}

#[derive(Debug, Reply, Serialize, Deserialize)]
pub(crate) struct BuffReport {
  pub(crate) player_id: PlayerId,
  pub(crate) state_after_buff: MapPlayerView,
}

#[remote_message("kameo_game_sync::MapActor::GivePlayerBuff")]
impl Message<GivePlayerBuff> for MapActor {
  type Reply = Option<BuffReport>;

  async fn handle(
    &mut self,
    GivePlayerBuff { player_id, buff }: GivePlayerBuff,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let player = self.players.get_mut(&player_id)?;
    player.state.add_buff(buff.clone());
    let state_after_buff = player.state.view();

    println!(
      "[map:{}] {} gained buff \"{}\" => attack=({},{}) damage=({},{}) buffs={}",
      self.map_id,
      state_after_buff.profile.name,
      buff.name,
      state_after_buff.effective_combat.attack.red,
      state_after_buff.effective_combat.attack.blue,
      state_after_buff.effective_combat.damage.red,
      state_after_buff.effective_combat.damage.blue,
      state_after_buff.buffs.len(),
    );

    self.broadcast_event(
      ctx.actor_ref(),
      player_id,
      MapEvent::PlayerStateChanged {
        map_id: self.map_id.clone(),
        player: state_after_buff.clone(),
      },
    );

    Some(BuffReport {
      player_id,
      state_after_buff,
    })
  }
}

// ── HitPlayer ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct HitPlayer {
  pub(crate) attacker_id: PlayerId,
  pub(crate) target_id: PlayerId,
  pub(crate) bonus_damage: Damage,
}

#[derive(Debug, Reply, Serialize, Deserialize)]
pub(crate) struct HitReport {
  pub(crate) attacker_id: PlayerId,
  pub(crate) target_id: PlayerId,
  pub(crate) effective_combat: EffectiveCombat,
  pub(crate) total_damage: Damage,
  pub(crate) state_after_hit: MapPlayerView,
}

#[remote_message("kameo_game_sync::MapActor::HitPlayer")]
impl Message<HitPlayer> for MapActor {
  type Reply = Option<HitReport>;

  async fn handle(
    &mut self,
    HitPlayer {
      attacker_id,
      target_id,
      bonus_damage,
    }: HitPlayer,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let attacker = self.players.get(&attacker_id)?;
    let attacker_name = attacker.state.profile.name.clone();
    let effective_combat = attacker.state.effective_combat();
    let total_damage = effective_combat.total_damage(bonus_damage);

    let target = self.players.get_mut(&target_id)?;
    target.state.stats.apply_damage(total_damage);
    let state_after_hit = target.state.view();
    println!(
      "[map:{}] {} hit {}: attack=({},{}) damage=({},{}) bonus=({},{}) total=({},{}) => red={}, \
       blue={}",
      self.map_id,
      attacker_name,
      state_after_hit.profile.name,
      effective_combat.attack.red,
      effective_combat.attack.blue,
      effective_combat.damage.red,
      effective_combat.damage.blue,
      bonus_damage.red,
      bonus_damage.blue,
      total_damage.red,
      total_damage.blue,
      state_after_hit.stats.red,
      state_after_hit.stats.blue,
    );

    self.broadcast_event(
      ctx.actor_ref(),
      target_id,
      MapEvent::PlayerStateChanged {
        map_id: self.map_id.clone(),
        player: state_after_hit.clone(),
      },
    );

    Some(HitReport {
      attacker_id,
      target_id,
      effective_combat,
      total_damage,
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
        "[map:{}] {} disconnected - grace period: 180 s",
        self.map_id, player.state.profile.name
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
        "[map:{}] {} evicted after 180 s disconnect timeout",
        self.map_id, player.state.profile.name
      );

      self.broadcast_event(
        ctx.actor_ref(),
        player_id,
        MapEvent::PlayerLeft {
          map_id: self.map_id.clone(),
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
  pub(crate) fn new(config: MapConfig) -> Self {
    Self {
      map_id: config.id,
      entry_buffs: config.entry_buffs,
      players: HashMap::new(),
    }
  }

  fn broadcast_event(&self, map_ref: &ActorRef<Self>, source_player_id: PlayerId, event: MapEvent) {
    for player in self.players.values() {
      let player_id = player.state.profile.id;
      if matches!(event, MapEvent::PlayerJoined { .. }) && player_id == source_player_id {
        continue;
      }
      let player_name = player.player_name.clone();
      let event = event.clone();
      let map_ref = map_ref.clone();
      let map_id = self.map_id.clone();

      tokio::spawn(async move {
        match RemoteActorRef::<PlayerActor>::lookup(player_name.as_str()).await {
          Ok(Some(remote_ref)) => {
            if let Err(e) = remote_ref.ask(&ApplyMapEvent { event }).await {
              println!("[map:{map_id}] event push failed for {player_name}: {e}");
              let _ = map_ref.tell(PlayerDisconnected { player_id }).await;
            }
          }
          Ok(None) => {
            println!("[map:{map_id}] {player_name} not in registry - marking offline");
            let _ = map_ref.tell(PlayerDisconnected { player_id }).await;
          }
          Err(e) => println!("[map:{map_id}] lookup error for {player_name}: {e}"),
        }
      });
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn player_buffs_stack_attack_damage_and_hit_damage() {
    let mut state = MapPlayerState {
      profile: PlayerProfile {
        id: 1,
        name: "Tester".to_string(),
      },
      stats: MapStats { red: 100, blue: 80 },
      base_attack: Damage { red: 10, blue: 3 },
      base_damage: Damage { red: 2, blue: 4 },
      buffs: vec![PlayerBuff::new(
        "map-aura",
        Damage { red: 1, blue: 2 },
        Damage { red: 3, blue: 0 },
      )],
    };

    state.add_buff(PlayerBuff::new(
      "manual-stack",
      Damage { red: 4, blue: 1 },
      Damage { red: 0, blue: 6 },
    ));

    let effective = state.effective_combat();
    assert_eq!(effective.attack, Damage { red: 15, blue: 6 });
    assert_eq!(effective.damage, Damage { red: 5, blue: 10 });

    let total_damage = effective.total_damage(Damage { red: 2, blue: 1 });
    assert_eq!(total_damage, Damage { red: 22, blue: 17 });

    state.stats.apply_damage(total_damage);
    assert_eq!(state.stats, MapStats { red: 78, blue: 63 });
  }
}
