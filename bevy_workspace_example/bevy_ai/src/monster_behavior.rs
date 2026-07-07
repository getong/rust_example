use bevior_tree::prelude::*;
use bevy::prelude::*;

use crate::{
  actors::{ArenaPosition, Monster, Player, RedBlueValues},
  config::MONSTER_ATTACK_RANGE,
  terrain::TerrainMap,
};

#[derive(Component, Clone, Debug)]
#[component(storage = "SparseSet")]
pub(crate) struct ChasePlayer {
  target: Entity,
}

pub(crate) fn monster_behavior_tree(
  target: Entity,
  tree_assets: &mut Assets<BehaviorTreeRoot>,
) -> BehaviorTree {
  BehaviorTree::from_node(
    InfiniteLoop::new(Sequence::new(vec![
      Box::new(TaskBridge::new(Box::new(WaitUntilChaseNeeded { target }))),
      Box::new(TaskBridge::new(Box::new(ChasePlayerTask { target }))),
    ])),
    tree_assets,
  )
}

pub(crate) fn move_chasing_monsters(
  time: Res<Time>,
  terrain_map: Res<TerrainMap>,
  players: Query<(&ArenaPosition, &RedBlueValues), With<Player>>,
  mut monsters: Query<
    (&mut ArenaPosition, &Monster, &RedBlueValues, &ChasePlayer),
    Without<Player>,
  >,
) {
  for (mut monster_position, monster, monster_values, chase) in &mut monsters {
    let Ok((target_position, target_values)) = players.get(chase.target) else {
      continue;
    };

    if !chase_is_needed(
      monster_position.0,
      *monster_values,
      target_position.0,
      *target_values,
      &terrain_map,
    ) {
      continue;
    }

    let waypoint = terrain_map.next_waypoint(monster_position.0, target_position.0);
    let to_waypoint = waypoint - monster_position.0;
    if to_waypoint.length() <= 1.0 {
      continue;
    }

    let movement = to_waypoint.normalize_or_zero() * monster.speed * time.delta_secs();
    monster_position.0 = terrain_map.try_move(monster_position.0, movement);
  }
}

#[derive(Debug)]
struct WaitUntilChaseNeeded {
  target: Entity,
}

impl TaskDefinition for WaitUntilChaseNeeded {
  fn build_checker(&self) -> Box<TaskChecker> {
    let target = self.target;
    Box::new(IntoSystem::into_system(
      move |In(entity): In<Entity>,
            terrain_map: Res<TerrainMap>,
            actors: Query<(&ArenaPosition, &RedBlueValues)>|
            -> TaskStatus {
        if entity_should_chase(entity, target, &terrain_map, &actors) {
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
struct ChasePlayerTask {
  target: Entity,
}

impl TaskDefinition for ChasePlayerTask {
  fn build_checker(&self) -> Box<TaskChecker> {
    let target = self.target;
    Box::new(IntoSystem::into_system(
      move |In(entity): In<Entity>,
            terrain_map: Res<TerrainMap>,
            actors: Query<(&ArenaPosition, &RedBlueValues)>|
            -> TaskStatus {
        if entity_should_chase(entity, target, &terrain_map, &actors) {
          TaskStatus::Running
        } else {
          TaskStatus::Complete(NodeResult::Success)
        }
      },
    ))
  }

  fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<TaskEventListener>)> {
    insert_while_running(ChasePlayer {
      target: self.target,
    })
  }
}

fn entity_should_chase(
  entity: Entity,
  target: Entity,
  terrain_map: &TerrainMap,
  actors: &Query<(&ArenaPosition, &RedBlueValues)>,
) -> bool {
  let Ok((monster_position, monster_values)) = actors.get(entity) else {
    return false;
  };
  let Ok((target_position, target_values)) = actors.get(target) else {
    return false;
  };

  chase_is_needed(
    monster_position.0,
    *monster_values,
    target_position.0,
    *target_values,
    terrain_map,
  )
}

fn chase_is_needed(
  monster_position: Vec2,
  monster_values: RedBlueValues,
  target_position: Vec2,
  target_values: RedBlueValues,
  terrain_map: &TerrainMap,
) -> bool {
  if monster_values.blue <= 0 || target_values.blue <= 0 {
    return false;
  }

  monster_position.distance(target_position) > MONSTER_ATTACK_RANGE
    || !terrain_map.segment_is_walkable(monster_position, target_position)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn behavior_tree_marks_monster_only_while_chasing() {
    let mut app = App::new();
    app.add_plugins((
      MinimalPlugins,
      BehaviorTreePlugin::default().in_schedule(Update),
    ));
    app.insert_resource(TerrainMap::default());

    let player = app
      .world_mut()
      .spawn((
        ArenaPosition(Vec2::ZERO),
        RedBlueValues { red: 18, blue: 140 },
        Player,
      ))
      .id();

    let monster =
      app
        .world_mut()
        .resource_scope(|world, mut tree_assets: Mut<Assets<BehaviorTreeRoot>>| {
          world
            .spawn((
              ArenaPosition(Vec2::new(MONSTER_ATTACK_RANGE + 10.0, 0.0)),
              RedBlueValues { red: 9, blue: 56 },
              Monster { speed: 92.0 },
              monster_behavior_tree(player, &mut tree_assets),
            ))
            .id()
        });

    app.update();
    assert!(app.world().entity(monster).contains::<ChasePlayer>());

    app
      .world_mut()
      .entity_mut(monster)
      .insert(ArenaPosition(Vec2::ZERO));

    app.update();
    assert!(!app.world().entity(monster).contains::<ChasePlayer>());
  }
}
