//! Map process actor — the authoritative ledger for map-local player state.
//!
//! The map process is formed by two halves running on the same node:
//! - [`MapActor`]: owns combat-buff instances and the per-player projection
//!   (profile + latest vitals). All stacking/expiry rules settle here.
//! - The Bevy world: the settlement engine for physics/combat. It receives the
//!   derived [`EffectiveModifiers`] through [`WorldCommand`] and reports
//!   vitals changes back via [`UpdateVitals`].
//!
//! Player actors may live on another node: entry, buff requests and leave are
//! `remote_message`s carrying only serializable data, and event broadcasts
//! fall back to a distributed-registry lookup when no local link is attached.

use std::collections::HashMap;

use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::actor::{
  player::{ApplyMapEvent, PlayerActor},
  types::{
    CombatBuff, EffectiveModifiers, MapEvent, MapPlayerView, MapVitals, PlayerId, PlayerProfile,
    WorldCommand,
  },
};

#[derive(Actor, RemoteActor)]
#[remote_actor(id = "game_server::MapActor")]
pub(crate) struct MapActor {
  map_id: String,
  entry_buffs: Vec<CombatBuff>,
  server_tick: u64,
  players: HashMap<PlayerId, MapPlayer>,
  world_tx: Option<UnboundedSender<WorldCommand>>,
}

struct MapPlayer {
  registry_name: String,
  link: Option<ActorRef<PlayerActor>>,
  profile: PlayerProfile,
  vitals: MapVitals,
  buffs: Vec<CombatBuff>,
}

impl MapPlayer {
  fn view(&self) -> MapPlayerView {
    MapPlayerView {
      profile: self.profile.clone(),
      vitals: self.vitals,
      buffs: self.buffs.clone(),
      effective: EffectiveModifiers::from_buffs(&self.buffs),
    }
  }

  fn upsert_buff(&mut self, buff: CombatBuff) {
    if let Some(existing) = self.buffs.iter_mut().find(|b| b.id == buff.id) {
      *existing = buff;
    } else {
      self.buffs.push(buff);
    }
  }
}

impl MapActor {
  pub(crate) fn new(
    map_id: impl Into<String>,
    entry_buffs: Vec<CombatBuff>,
    world_tx: Option<UnboundedSender<WorldCommand>>,
  ) -> Self {
    Self {
      map_id: map_id.into(),
      entry_buffs,
      server_tick: 0,
      players: HashMap::new(),
      world_tx,
    }
  }

  fn push_modifiers(&self, player_id: PlayerId) {
    let Some(world_tx) = self.world_tx.as_ref() else {
      return;
    };
    let Some(player) = self.players.get(&player_id) else {
      return;
    };
    let _ = world_tx.send(WorldCommand::UpdateModifiers {
      player_id,
      effective: EffectiveModifiers::from_buffs(&player.buffs),
    });
  }

  fn broadcast(&self, event: MapEvent) {
    for player in self.players.values() {
      let event = event.clone();
      if let Some(link) = player.link.clone() {
        tokio::spawn(async move {
          let _ = link.tell(ApplyMapEvent { event }).await;
        });
        continue;
      }

      // No local link: the player process lives on another node, reach it via
      // the kameo remote registry.
      let registry_name = player.registry_name.clone();
      let map_id = self.map_id.clone();
      tokio::spawn(async move {
        match RemoteActorRef::<PlayerActor>::lookup(registry_name.as_str()).await {
          Ok(Some(remote_ref)) => {
            if let Err(err) = remote_ref.tell(&ApplyMapEvent { event }).send() {
              eprintln!("map:{map_id} event push to {registry_name} failed: {err}");
            }
          }
          Ok(None) => {}
          Err(err) => eprintln!("map:{map_id} lookup of {registry_name} failed: {err}"),
        }
      });
    }
  }
}

// ── EnterPlayer (remote-capable) ─────────────────────────────────────────────

/// Sent by a player process when entering the map. Carries the profile
/// projection plus any portable buffs banked from a previous map: this is the
/// ownership handoff — from now on the map is the only writer of these buffs.
#[derive(Serialize, Deserialize)]
pub(crate) struct EnterPlayer {
  pub(crate) profile: PlayerProfile,
  pub(crate) registry_name: String,
  pub(crate) vitals: MapVitals,
  pub(crate) portable_buffs: Vec<CombatBuff>,
}

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct MapJoinInfo {
  pub(crate) map_id: String,
  pub(crate) tick: u64,
  pub(crate) own: MapPlayerView,
  pub(crate) others: Vec<MapPlayerView>,
}

#[remote_message("game_server::MapActor::EnterPlayer")]
impl Message<EnterPlayer> for MapActor {
  type Reply = MapJoinInfo;

  async fn handle(
    &mut self,
    EnterPlayer {
      profile,
      registry_name,
      vitals,
      portable_buffs,
    }: EnterPlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let player_id = profile.id;
    let others: Vec<MapPlayerView> = self
      .players
      .values()
      .filter(|p| p.profile.id != player_id)
      .map(MapPlayer::view)
      .collect();

    let player = self
      .players
      .entry(player_id)
      .or_insert_with(|| MapPlayer {
        registry_name: registry_name.clone(),
        link: None,
        profile: profile.clone(),
        vitals,
        buffs: self.entry_buffs.clone(),
      });
    player.registry_name = registry_name;
    player.profile = profile;
    for buff in portable_buffs {
      player.upsert_buff(buff);
    }

    let own = player.view();
    self.push_modifiers(player_id);
    self.broadcast(MapEvent::Entered {
      map_id: self.map_id.clone(),
      tick: self.server_tick,
      player: own.clone(),
    });

    MapJoinInfo {
      map_id: self.map_id.clone(),
      tick: self.server_tick,
      own,
      others,
    }
  }
}

// ── AttachLocalPlayer (local-only fast path) ─────────────────────────────────

/// Upgrades a player's event link to a direct local `ActorRef`, skipping the
/// remote registry when both actors run on this node.
pub(crate) struct AttachLocalPlayer {
  pub(crate) player_id: PlayerId,
  pub(crate) player: ActorRef<PlayerActor>,
}

impl Message<AttachLocalPlayer> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    AttachLocalPlayer { player_id, player }: AttachLocalPlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    if let Some(entry) = self.players.get_mut(&player_id) {
      entry.link = Some(player);
    }
  }
}

// ── ApplyCombatBuff (remote-capable) ─────────────────────────────────────────

/// Requests a combat buff for a player. The source may be anyone (player item,
/// map aura, another system) but the instance is created here and only here.
#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyCombatBuff {
  pub(crate) player_id: PlayerId,
  pub(crate) buff: CombatBuff,
}

#[remote_message("game_server::MapActor::ApplyCombatBuff")]
impl Message<ApplyCombatBuff> for MapActor {
  type Reply = Option<MapPlayerView>;

  async fn handle(
    &mut self,
    ApplyCombatBuff { player_id, buff }: ApplyCombatBuff,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let player = self.players.get_mut(&player_id)?;
    player.upsert_buff(buff);
    let view = player.view();

    self.push_modifiers(player_id);
    self.broadcast(MapEvent::StateChanged {
      map_id: self.map_id.clone(),
      tick: self.server_tick,
      player: view.clone(),
    });
    Some(view)
  }
}

// ── LeavePlayer (remote-capable) ─────────────────────────────────────────────

/// Removes the player and hands portable buff instances back to the player
/// process (the reverse ownership handoff for map transfer / disconnect).
#[derive(Serialize, Deserialize)]
pub(crate) struct LeavePlayer {
  pub(crate) player_id: PlayerId,
}

#[remote_message("game_server::MapActor::LeavePlayer")]
impl Message<LeavePlayer> for MapActor {
  type Reply = Option<Vec<CombatBuff>>;

  async fn handle(
    &mut self,
    LeavePlayer { player_id }: LeavePlayer,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let player = self.players.remove(&player_id)?;
    self.broadcast(MapEvent::Left {
      map_id: self.map_id.clone(),
      tick: self.server_tick,
      player_id,
    });
    Some(
      player
        .buffs
        .into_iter()
        .filter(|buff| buff.portable)
        .collect(),
    )
  }
}

// ── UpdateVitals (local-only, from the ECS settlement engine) ────────────────

/// The Bevy world is the single writer of vitals; the map ledger mirrors the
/// latest value and fans it out to player processes.
pub(crate) struct UpdateVitals {
  pub(crate) player_id: PlayerId,
  pub(crate) vitals: MapVitals,
}

impl Message<UpdateVitals> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    UpdateVitals { player_id, vitals }: UpdateVitals,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let Some(player) = self.players.get_mut(&player_id) else {
      return;
    };
    if player.vitals == vitals {
      return;
    }
    player.vitals = vitals;
    let view = player.view();
    self.broadcast(MapEvent::StateChanged {
      map_id: self.map_id.clone(),
      tick: self.server_tick,
      player: view,
    });
  }
}

// ── AdvanceTick (local-only, buff duration bookkeeping) ──────────────────────

/// Combat buffs are measured in server ticks, so the map clock is the ECS
/// tick counter forwarded at snapshot cadence.
pub(crate) struct AdvanceTick {
  pub(crate) tick: u64,
}

impl Message<AdvanceTick> for MapActor {
  type Reply = ();

  async fn handle(
    &mut self,
    AdvanceTick { tick }: AdvanceTick,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let delta = tick.saturating_sub(self.server_tick);
    self.server_tick = tick;
    if delta == 0 {
      return;
    }

    let mut expired: Vec<PlayerId> = Vec::new();
    for (player_id, player) in &mut self.players {
      let before = player.buffs.len();
      for buff in &mut player.buffs {
        if let Some(remaining) = buff.remaining_ticks.as_mut() {
          *remaining = remaining.saturating_sub(delta);
        }
      }
      player.buffs.retain(|buff| buff.remaining_ticks != Some(0));
      if player.buffs.len() != before {
        expired.push(*player_id);
      }
    }

    for player_id in expired {
      self.push_modifiers(player_id);
      if let Some(player) = self.players.get(&player_id) {
        self.broadcast(MapEvent::StateChanged {
          map_id: self.map_id.clone(),
          tick: self.server_tick,
          player: player.view(),
        });
      }
    }
  }
}

// ── GetAllPlayers (remote-capable query) ─────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GetAllPlayers;

#[remote_message("game_server::MapActor::GetAllPlayers")]
impl Message<GetAllPlayers> for MapActor {
  type Reply = Vec<MapPlayerView>;

  async fn handle(
    &mut self,
    _msg: GetAllPlayers,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.players.values().map(MapPlayer::view).collect()
  }
}

#[cfg(test)]
mod tests {
  use tokio::sync::mpsc;

  use super::*;

  fn profile(id: PlayerId) -> PlayerProfile {
    PlayerProfile {
      id,
      name: format!("Player {id}"),
      room: None,
    }
  }

  const VITALS: MapVitals = MapVitals { red: 18, blue: 140 };

  #[tokio::test]
  async fn map_owns_buff_instances_and_pushes_derived_modifiers() {
    let (world_tx, mut world_rx) = mpsc::unbounded_channel();
    let map = MapActor::spawn(MapActor::new(
      "arena",
      vec![CombatBuff::permanent("arena-ward", 0.9)],
      Some(world_tx),
    ));

    // Entry: map grants its own aura buff and merges the portable buff the
    // player carried over from a previous map.
    let join = map
      .ask(EnterPlayer {
        profile: profile(7),
        registry_name: "player:7".into(),
        vitals: VITALS,
        portable_buffs: vec![CombatBuff {
          id: "haste".into(),
          damage_taken_mult: 1.0,
          move_speed_mult: 1.25,
          remaining_ticks: Some(10),
          portable: true,
        }],
      })
      .await
      .unwrap();
    assert_eq!(join.own.buffs.len(), 2);
    assert!((join.own.effective.damage_taken_mult - 0.9).abs() < 1e-6);
    assert!((join.own.effective.move_speed_mult - 1.25).abs() < 1e-6);

    let WorldCommand::UpdateModifiers {
      player_id,
      effective,
    } = world_rx.recv().await.unwrap();
    assert_eq!(player_id, 7);
    assert!((effective.damage_taken_mult - 0.9).abs() < 1e-6);

    // A player-sourced buff request still creates the instance in the map.
    let view = map
      .ask(ApplyCombatBuff {
        player_id: 7,
        buff: CombatBuff {
          id: "iron-skin".into(),
          damage_taken_mult: 0.5,
          move_speed_mult: 1.0,
          remaining_ticks: Some(5),
          portable: false,
        },
      })
      .await
      .unwrap()
      .unwrap();
    assert_eq!(view.buffs.len(), 3);
    assert!((view.effective.damage_taken_mult - 0.45).abs() < 1e-6);
    let _ = world_rx.recv().await.unwrap();

    // Tick expiry: iron-skin (5 ticks) dies, haste (10 ticks) survives.
    map.ask(AdvanceTick { tick: 6 }).await.unwrap();
    let players = map.ask(GetAllPlayers).await.unwrap();
    let buff_ids: Vec<&str> = players[0].buffs.iter().map(|b| b.id.as_str()).collect();
    assert!(buff_ids.contains(&"arena-ward"));
    assert!(buff_ids.contains(&"haste"));
    assert!(!buff_ids.contains(&"iron-skin"));
    let _ = world_rx.recv().await.unwrap();

    // Leave: only portable buffs are handed back for the next map.
    let portable = map.ask(LeavePlayer { player_id: 7 }).await.unwrap().unwrap();
    assert_eq!(portable.len(), 1);
    assert_eq!(portable[0].id, "haste");
    assert_eq!(portable[0].remaining_ticks, Some(4));
  }
}
