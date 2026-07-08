use bevior_tree::prelude::*;
use bevy::prelude::*;

use crate::{
  game::{ArenaPosition, MONSTER_ATTACK_RANGE, Monster, Player, Vitals},
  terrain::TerrainMap,
};

#[derive(Component, Clone, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct ChaseNearestPlayer;

pub(crate) fn monster_behavior_tree(tree_assets: &mut Assets<BehaviorTreeRoot>) -> BehaviorTree {
  BehaviorTree::from_node(
    InfiniteLoop::new(Sequence::new(vec![
      Box::new(TaskBridge::new(Box::new(WaitUntilChaseNeeded))),
      Box::new(TaskBridge::new(Box::new(ChaseNearestPlayerTask))),
    ])),
    tree_assets,
  )
}

pub(crate) fn move_chasing_monsters(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  players: Query<(&ArenaPosition, &Vitals), With<Player>>,
  mut monsters: Query<
    (&mut ArenaPosition, &Monster, &Vitals),
    (With<ChaseNearestPlayer>, Without<Player>),
  >,
) {
  for (mut monster_position, monster, monster_vitals) in &mut monsters {
    if monster_vitals.blue <= 0 {
      continue;
    }

    let Some(target_position) = nearest_alive_player(monster_position.0, &players) else {
      continue;
    };

    let to_target = horizontal_delta(monster_position.0, target_position);
    if to_target.length() <= MONSTER_ATTACK_RANGE {
      continue;
    }

    let waypoint = terrain_map.next_waypoint(monster_position.0, target_position);
    let to_waypoint = horizontal_delta(monster_position.0, waypoint);
    if to_waypoint.length() <= 1.0 {
      continue;
    }

    let movement = to_waypoint.normalize_or_zero() * monster.speed * time.delta_secs();
    monster_position.0 = terrain_map.try_move(monster_position.0, movement);
  }
}

#[derive(Debug)]
struct WaitUntilChaseNeeded;

impl TaskDefinition for WaitUntilChaseNeeded {
  fn build_checker(&self) -> Box<TaskChecker> {
    Box::new(IntoSystem::into_system(
      |In(entity): In<Entity>,
       terrain_map: Res<TerrainMap>,
       actors: Query<(&ArenaPosition, &Vitals, Option<&Player>, Option<&Monster>)>|
       -> TaskStatus {
        if monster_should_chase(entity, &terrain_map, &actors) {
          TaskStatus::Complete(NodeResult::Success)
        } else {
          TaskStatus::Running
        }
      },
    ))
  }

  fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<TaskEventListener>)> {
    vec![]
  }
}

#[derive(Debug)]
struct ChaseNearestPlayerTask;

impl TaskDefinition for ChaseNearestPlayerTask {
  fn build_checker(&self) -> Box<TaskChecker> {
    Box::new(IntoSystem::into_system(
      |In(entity): In<Entity>,
       terrain_map: Res<TerrainMap>,
       actors: Query<(&ArenaPosition, &Vitals, Option<&Player>, Option<&Monster>)>|
       -> TaskStatus {
        if monster_should_chase(entity, &terrain_map, &actors) {
          TaskStatus::Running
        } else {
          TaskStatus::Complete(NodeResult::Success)
        }
      },
    ))
  }

  fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<TaskEventListener>)> {
    insert_while_running(ChaseNearestPlayer)
  }
}

fn monster_should_chase(
  entity: Entity,
  terrain_map: &TerrainMap,
  actors: &Query<(&ArenaPosition, &Vitals, Option<&Player>, Option<&Monster>)>,
) -> bool {
  let Ok((monster_position, monster_vitals, _, Some(_))) = actors.get(entity) else {
    return false;
  };
  if monster_vitals.blue <= 0 {
    return false;
  }

  let Some(target_position) = actors
    .iter()
    .filter_map(|(position, vitals, player, _)| {
      (player.is_some() && vitals.blue > 0).then_some(position.0)
    })
    .min_by(|left, right| {
      horizontal_distance_squared(monster_position.0, *left)
        .total_cmp(&horizontal_distance_squared(monster_position.0, *right))
    })
  else {
    return false;
  };

  horizontal_distance(monster_position.0, target_position) > MONSTER_ATTACK_RANGE
    || !terrain_map.segment_is_walkable(monster_position.0, target_position)
}

fn nearest_alive_player(
  monster_position: Vec3,
  players: &Query<(&ArenaPosition, &Vitals), With<Player>>,
) -> Option<Vec3> {
  players
    .iter()
    .filter_map(|(position, vitals)| (vitals.blue > 0).then_some(position.0))
    .min_by(|left, right| {
      horizontal_distance_squared(monster_position, *left)
        .total_cmp(&horizontal_distance_squared(monster_position, *right))
    })
}

fn horizontal_delta(from: Vec3, to: Vec3) -> Vec3 {
  Vec3::new(to.x - from.x, 0.0, to.z - from.z)
}

fn horizontal_distance(from: Vec3, to: Vec3) -> f32 {
  horizontal_delta(from, to).length()
}

fn horizontal_distance_squared(from: Vec3, to: Vec3) -> f32 {
  horizontal_delta(from, to).length_squared()
}
