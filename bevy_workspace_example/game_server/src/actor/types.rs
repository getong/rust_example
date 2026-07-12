//! Shared value types exchanged between the player process and the map
//! process.
//!
//! Ownership rules (single writer per field):
//! - Combat buffs settle in the map tick, so their instances live in
//!   [`crate::actor::map::MapActor`]. Player processes may *request* them but
//!   never create instances locally.
//! - Business buffs (experience multipliers, ...) settle in the player
//!   process, so their instances live in [`crate::actor::player::PlayerActor`].
//! - [`EffectiveModifiers`] is a derived cache: computed only by the map actor
//!   and pushed into the Bevy world through [`WorldCommand`]. Nobody else may
//!   recompute it, so the stacking formula exists in exactly one place.

use kameo::prelude::*;
use serde::{Deserialize, Serialize};

pub(crate) type PlayerId = u64;

#[derive(Clone, Debug, PartialEq, Eq, Reply, Serialize, Deserialize)]
pub(crate) struct PlayerProfile {
  pub(crate) id: PlayerId,
  pub(crate) name: String,
  pub(crate) room: Option<String>,
}

/// A combat buff instance. Owned by the map actor; anything a player process
/// holds is either a display mirror or a banked portable buff waiting for the
/// next map entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CombatBuff {
  pub(crate) id: String,
  /// Multiplier applied to incoming damage (0.9 = 10% damage reduction).
  pub(crate) damage_taken_mult: f32,
  /// Multiplier applied to movement speed.
  pub(crate) move_speed_mult: f32,
  /// Remaining duration in server ticks; `None` means "until leaving the map".
  pub(crate) remaining_ticks: Option<u64>,
  /// Whether the buff survives a map transfer / disconnect handoff.
  pub(crate) portable: bool,
}

impl CombatBuff {
  pub(crate) fn permanent(id: impl Into<String>, damage_taken_mult: f32) -> Self {
    Self {
      id: id.into(),
      damage_taken_mult,
      move_speed_mult: 1.0,
      remaining_ticks: None,
      portable: false,
    }
  }
}

/// A business buff instance. Owned by the player actor; the map never sees it
/// because its settlement point (experience gain, drop rates, ...) is the
/// player process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BusinessBuff {
  pub(crate) id: String,
  pub(crate) exp_mult: f32,
  /// Wall-clock expiry in unix milliseconds; business buffs keep running while
  /// the player is off-map, so they use wall time instead of map ticks.
  pub(crate) expires_at_ms: Option<u64>,
}

impl BusinessBuff {
  pub(crate) fn active_at(&self, now_ms: u64) -> bool {
    self.expires_at_ms.is_none_or(|expiry| now_ms < expiry)
  }
}

/// Aggregated combat modifiers, derived from the map-owned buff list. The
/// stacking formula lives here and is invoked only by the map actor.
#[derive(Clone, Copy, Debug, PartialEq, Reply, Serialize, Deserialize)]
pub(crate) struct EffectiveModifiers {
  pub(crate) damage_taken_mult: f32,
  pub(crate) move_speed_mult: f32,
}

impl Default for EffectiveModifiers {
  fn default() -> Self {
    Self {
      damage_taken_mult: 1.0,
      move_speed_mult: 1.0,
    }
  }
}

impl EffectiveModifiers {
  pub(crate) fn from_buffs(buffs: &[CombatBuff]) -> Self {
    buffs.iter().fold(Self::default(), |acc, buff| Self {
      damage_taken_mult: acc.damage_taken_mult * buff.damage_taken_mult,
      move_speed_mult: acc.move_speed_mult * buff.move_speed_mult,
    })
  }
}

/// Mirror of the ECS-owned vitals; the Bevy world is the settlement engine and
/// only writer, the map actor keeps the latest copy for bookkeeping and player
/// mirror broadcasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reply, Serialize, Deserialize)]
pub(crate) struct MapVitals {
  pub(crate) red: i32,
  pub(crate) blue: i32,
}

/// Read-only projection of one player's map-owned state.
#[derive(Clone, Debug, PartialEq, Reply, Serialize, Deserialize)]
pub(crate) struct MapPlayerView {
  pub(crate) profile: PlayerProfile,
  pub(crate) vitals: MapVitals,
  pub(crate) buffs: Vec<CombatBuff>,
  pub(crate) effective: EffectiveModifiers,
}

/// Facts pushed from the authoritative map to player processes. Players only
/// refresh their display/recovery mirror from these; they never settle
/// anything from them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum MapEvent {
  Entered {
    map_id: String,
    tick: u64,
    player: MapPlayerView,
  },
  StateChanged {
    map_id: String,
    tick: u64,
    player: MapPlayerView,
  },
  Left {
    map_id: String,
    tick: u64,
    player_id: PlayerId,
  },
}

/// Commands from the actor layer into the Bevy world (drained once per tick).
/// This is the only path that writes derived combat state into ECS components.
#[derive(Clone, Debug)]
pub(crate) enum WorldCommand {
  UpdateModifiers {
    player_id: PlayerId,
    effective: EffectiveModifiers,
  },
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn effective_modifiers_multiply_across_buffs() {
    let buffs = vec![
      CombatBuff::permanent("ward", 0.9),
      CombatBuff {
        id: "haste".into(),
        damage_taken_mult: 1.0,
        move_speed_mult: 1.25,
        remaining_ticks: Some(300),
        portable: true,
      },
    ];

    let effective = EffectiveModifiers::from_buffs(&buffs);
    assert!((effective.damage_taken_mult - 0.9).abs() < f32::EPSILON);
    assert!((effective.move_speed_mult - 1.25).abs() < f32::EPSILON);
  }

  #[test]
  fn business_buff_expiry_uses_wall_clock() {
    let buff = BusinessBuff {
      id: "double-exp".into(),
      exp_mult: 2.0,
      expires_at_ms: Some(1_000),
    };
    assert!(buff.active_at(999));
    assert!(!buff.active_at(1_000));
  }
}
