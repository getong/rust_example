//! Player process actor — one kameo actor per connected player.
//!
//! Owns (single writer):
//! - the player profile and session-level business state (experience),
//! - business buff instances (experience multipliers settle here, so their
//!   instances live here — the mirror-image of combat buffs living in the
//!   map),
//! - portable combat buffs *banked while off-map* (between LeaveMap and the
//!   next EnterMap the player process is the temporary sole holder, which is
//!   what makes the map-transfer handoff race-free).
//!
//! Mirrors (read-only, refreshed by [`ApplyMapEvent`]):
//! - the map-owned state (vitals, combat buffs, effective modifiers) for
//!   display, persistence and crash recovery. Never used for settlement.

use kameo::{actor::RemoteActorRef, prelude::*};
use serde::{Deserialize, Serialize};

use crate::actor::{
  map::{ApplyCombatBuff, EnterPlayer, LeavePlayer, MapActor, MapJoinInfo},
  types::{BusinessBuff, CombatBuff, MapEvent, MapPlayerView, MapVitals, PlayerProfile},
};

#[derive(Actor, RemoteActor)]
#[remote_actor(id = "game_server::PlayerActor")]
pub(crate) struct PlayerActor {
  profile: PlayerProfile,
  registry_name: String,
  experience: u64,
  business_buffs: Vec<BusinessBuff>,
  banked_buffs: Vec<CombatBuff>,
  map_link: Option<MapLink>,
  mirror: Option<MapMirror>,
}

/// Local fast path when the map runs on this node, remote ref otherwise.
#[derive(Clone)]
pub(crate) enum MapLink {
  Local(ActorRef<MapActor>),
  Remote(RemoteActorRef<MapActor>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MapMirror {
  pub(crate) map_id: String,
  pub(crate) tick: u64,
  pub(crate) own: MapPlayerView,
  pub(crate) others: Vec<MapPlayerView>,
}

impl PlayerActor {
  pub(crate) fn new(profile: PlayerProfile, registry_name: impl Into<String>) -> Self {
    Self {
      profile,
      registry_name: registry_name.into(),
      experience: 0,
      business_buffs: Vec::new(),
      banked_buffs: Vec::new(),
      map_link: None,
      mirror: None,
    }
  }
}

// ── EnterMap (local-only orchestration) ──────────────────────────────────────

/// Joins a map, handing banked portable combat buffs over to the map — after
/// the reply returns, the map is their only writer.
pub(crate) struct EnterMap {
  pub(crate) map: MapLink,
  pub(crate) vitals: MapVitals,
}

impl Message<EnterMap> for PlayerActor {
  type Reply = Option<MapJoinInfo>;

  async fn handle(
    &mut self,
    EnterMap { map, vitals }: EnterMap,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let enter = EnterPlayer {
      profile: self.profile.clone(),
      registry_name: self.registry_name.clone(),
      vitals,
      portable_buffs: std::mem::take(&mut self.banked_buffs),
    };

    let join = match &map {
      MapLink::Local(map_ref) => map_ref.ask(enter).await.ok(),
      MapLink::Remote(map_ref) => map_ref.ask(&enter).await.ok(),
    };

    let Some(join) = join else {
      eprintln!(
        "player:{} enter map failed, keeping banked buffs empty-handed",
        self.profile.id
      );
      return None;
    };

    self.map_link = Some(map);
    self.mirror = Some(MapMirror {
      map_id: join.map_id.clone(),
      tick: join.tick,
      own: join.own.clone(),
      others: join.others.clone(),
    });
    Some(join)
  }
}

// ── LeaveMap (local-only orchestration) ──────────────────────────────────────

/// Leaves the current map and banks the portable combat buffs returned by the
/// map, ready for the next EnterMap (map transfer or reconnect).
pub(crate) struct LeaveMap;

impl Message<LeaveMap> for PlayerActor {
  type Reply = usize;

  async fn handle(&mut self, _msg: LeaveMap, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
    let Some(map) = self.map_link.take() else {
      return 0;
    };
    let leave = LeavePlayer {
      player_id: self.profile.id,
    };
    let portable = match &map {
      MapLink::Local(map_ref) => map_ref.ask(leave).await.ok().flatten(),
      MapLink::Remote(map_ref) => map_ref.ask(&leave).await.ok().flatten(),
    };
    self.mirror = None;
    if let Some(portable) = portable {
      self.banked_buffs = portable;
    }
    self.banked_buffs.len()
  }
}

// ── RequestCombatBuff (local-only) ───────────────────────────────────────────

/// A player-sourced combat buff (item use, skill, ...). The player process is
/// only the *source*: it forwards the request to the map, which owns the
/// instance. The local copy in the mirror arrives via ApplyMapEvent later.
pub(crate) struct RequestCombatBuff {
  pub(crate) buff: CombatBuff,
}

impl Message<RequestCombatBuff> for PlayerActor {
  type Reply = Option<MapPlayerView>;

  async fn handle(
    &mut self,
    RequestCombatBuff { buff }: RequestCombatBuff,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    let map = self.map_link.clone()?;
    let request = ApplyCombatBuff {
      player_id: self.profile.id,
      buff,
    };
    match &map {
      MapLink::Local(map_ref) => map_ref.ask(request).await.ok().flatten(),
      MapLink::Remote(map_ref) => map_ref.ask(&request).await.ok().flatten(),
    }
  }
}

// ── AddBusinessBuff (remote-capable) ─────────────────────────────────────────

/// Business buffs settle in the player process, so the instance is created
/// here — the mirror-image of ApplyCombatBuff on the map.
#[derive(Serialize, Deserialize)]
pub(crate) struct AddBusinessBuff {
  pub(crate) buff: BusinessBuff,
}

#[remote_message("game_server::PlayerActor::AddBusinessBuff")]
impl Message<AddBusinessBuff> for PlayerActor {
  type Reply = usize;

  async fn handle(
    &mut self,
    AddBusinessBuff { buff }: AddBusinessBuff,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    if let Some(existing) = self.business_buffs.iter_mut().find(|b| b.id == buff.id) {
      *existing = buff;
    } else {
      self.business_buffs.push(buff);
    }
    self.business_buffs.len()
  }
}

// ── GainExperience (remote-capable settlement) ───────────────────────────────

/// Experience settlement point. The map (or any kill source) reports the raw
/// amount; the player process applies its own business buffs. The map never
/// needs to know the multipliers — same isolation as combat settlement.
#[derive(Serialize, Deserialize)]
pub(crate) struct GainExperience {
  pub(crate) base: u64,
  pub(crate) now_ms: u64,
}

#[remote_message("game_server::PlayerActor::GainExperience")]
impl Message<GainExperience> for PlayerActor {
  type Reply = u64;

  async fn handle(
    &mut self,
    GainExperience { base, now_ms }: GainExperience,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self
      .business_buffs
      .retain(|buff| buff.active_at(now_ms) || buff.expires_at_ms.is_none());
    let mult: f32 = self
      .business_buffs
      .iter()
      .filter(|buff| buff.active_at(now_ms))
      .map(|buff| buff.exp_mult)
      .product();
    let gained = (base as f32 * mult.max(0.0)).round() as u64;
    self.experience += gained;
    gained
  }
}

// ── ApplyMapEvent (remote-capable mirror refresh) ────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyMapEvent {
  pub(crate) event: MapEvent,
}

#[remote_message("game_server::PlayerActor::ApplyMapEvent")]
impl Message<ApplyMapEvent> for PlayerActor {
  type Reply = ();

  async fn handle(
    &mut self,
    ApplyMapEvent { event }: ApplyMapEvent,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    match event {
      MapEvent::Entered {
        map_id,
        tick,
        player,
      }
      | MapEvent::StateChanged {
        map_id,
        tick,
        player,
      } => {
        let Some(mirror) = self.mirror.as_mut() else {
          return;
        };
        if mirror.map_id != map_id || tick < mirror.tick {
          return;
        }
        mirror.tick = tick;
        if player.profile.id == self.profile.id {
          mirror.own = player;
        } else if let Some(existing) = mirror
          .others
          .iter_mut()
          .find(|view| view.profile.id == player.profile.id)
        {
          *existing = player;
        } else {
          mirror.others.push(player);
        }
      }
      MapEvent::Left {
        map_id, player_id, ..
      } => {
        if let Some(mirror) = self.mirror.as_mut()
          && mirror.map_id == map_id
        {
          mirror.others.retain(|view| view.profile.id != player_id);
        }
      }
    }
  }
}

// ── GetPlayerView (remote-capable query) ─────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(crate) struct GetPlayerView;

#[derive(Clone, Debug, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerView {
  pub(crate) profile: PlayerProfile,
  pub(crate) experience: u64,
  pub(crate) business_buffs: Vec<BusinessBuff>,
  pub(crate) banked_buffs: Vec<CombatBuff>,
  pub(crate) mirror: Option<MapMirror>,
}

#[remote_message("game_server::PlayerActor::GetPlayerView")]
impl Message<GetPlayerView> for PlayerActor {
  type Reply = PlayerView;

  async fn handle(
    &mut self,
    _msg: GetPlayerView,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    PlayerView {
      profile: self.profile.clone(),
      experience: self.experience,
      business_buffs: self.business_buffs.clone(),
      banked_buffs: self.banked_buffs.clone(),
      mirror: self.mirror.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;
  use crate::actor::{
    map::AttachLocalPlayer,
    types::{CombatBuff, EffectiveModifiers, PlayerId},
  };

  fn profile(id: PlayerId) -> PlayerProfile {
    PlayerProfile {
      id,
      name: format!("Player {id}"),
      room: None,
    }
  }

  const VITALS: MapVitals = MapVitals { red: 18, blue: 140 };

  #[tokio::test]
  async fn business_buffs_settle_in_player_process() {
    let player = PlayerActor::spawn(PlayerActor::new(profile(1), "player:1"));

    player
      .ask(AddBusinessBuff {
        buff: BusinessBuff {
          id: "double-exp".into(),
          exp_mult: 2.0,
          expires_at_ms: Some(10_000),
        },
      })
      .await
      .unwrap();

    let gained = player
      .ask(GainExperience {
        base: 100,
        now_ms: 5_000,
      })
      .await
      .unwrap();
    assert_eq!(gained, 200);

    // Expired buff no longer applies.
    let gained = player
      .ask(GainExperience {
        base: 100,
        now_ms: 20_000,
      })
      .await
      .unwrap();
    assert_eq!(gained, 100);

    let view = player.ask(GetPlayerView).await.unwrap();
    assert_eq!(view.experience, 300);
  }

  #[tokio::test]
  async fn enter_map_hands_off_buffs_and_mirror_tracks_map_events() {
    let map = MapActor::spawn(MapActor::new(
      "arena",
      vec![CombatBuff::permanent("arena-ward", 0.9)],
      None,
    ));
    let player = PlayerActor::spawn(PlayerActor::new(profile(2), "player:2"));

    let join = player
      .ask(EnterMap {
        map: MapLink::Local(map.clone()),
        vitals: VITALS,
      })
      .await
      .unwrap()
      .expect("join succeeds");
    assert_eq!(join.map_id, "arena");
    assert_eq!(join.own.buffs.len(), 1);

    map
      .ask(AttachLocalPlayer {
        player_id: 2,
        player: player.clone(),
      })
      .await
      .unwrap();

    // Player-sourced combat buff: instance is created map-side, and the
    // mirror is refreshed by the map's broadcast, not by the player itself.
    let view = player
      .ask(RequestCombatBuff {
        buff: CombatBuff {
          id: "haste".into(),
          damage_taken_mult: 1.0,
          move_speed_mult: 1.25,
          remaining_ticks: Some(10),
          portable: true,
        },
      })
      .await
      .unwrap()
      .expect("buff applied");
    assert_eq!(view.buffs.len(), 2);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let player_view = player.ask(GetPlayerView).await.unwrap();
    let mirror = player_view.mirror.expect("mirror present");
    assert_eq!(mirror.own.buffs.len(), 2);
    assert_eq!(
      mirror.own.effective,
      EffectiveModifiers {
        damage_taken_mult: 0.9,
        move_speed_mult: 1.25,
      }
    );

    // Leave banks only the portable buff for the next map.
    let banked = player.ask(LeaveMap).await.unwrap();
    assert_eq!(banked, 1);
    let player_view = player.ask(GetPlayerView).await.unwrap();
    assert_eq!(player_view.banked_buffs[0].id, "haste");
    assert!(player_view.mirror.is_none());
  }
}
