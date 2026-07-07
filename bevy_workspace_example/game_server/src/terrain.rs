use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{
  game::ARENA_HALF_SIZE,
  protocol::{MapState, ObstacleShape, ObstacleState},
};

const CELL_SIZE: f32 = 40.0;
const ACTOR_RADIUS: f32 = 18.0;
const BOUNDARY_WALL_THICKNESS: f32 = 20.0;
const PLAYABLE_PADDING: f32 = ACTOR_RADIUS + BOUNDARY_WALL_THICKNESS;

#[derive(Resource, Debug, Clone)]
pub(crate) struct LevelMap {
  pub(crate) name: &'static str,
  pub(crate) player_spawn: Vec2,
  pub(crate) obstacles: Vec<TerrainSpec>,
  pub(crate) monsters: Vec<MonsterSpawn>,
}

impl Default for LevelMap {
  fn default() -> Self {
    Self {
      name: "Crossroads",
      player_spawn: Vec2::ZERO,
      obstacles: CROSSROADS_OBSTACLES.to_vec(),
      monsters: CROSSROADS_MONSTERS.to_vec(),
    }
  }
}

#[derive(Resource, Debug, Clone)]
pub(crate) struct TerrainMap {
  obstacles: Vec<TerrainSpec>,
  blocked: Vec<bool>,
  columns: i32,
  rows: i32,
}

impl Default for TerrainMap {
  fn default() -> Self {
    Self::for_level(&LevelMap::default())
  }
}

impl TerrainMap {
  pub(crate) fn for_level(map: &LevelMap) -> Self {
    let columns = ((ARENA_HALF_SIZE.x * 2.0) / CELL_SIZE).ceil() as i32;
    let rows = ((ARENA_HALF_SIZE.y * 2.0) / CELL_SIZE).ceil() as i32;
    let obstacles = map.obstacles.clone();
    let mut blocked = Vec::with_capacity((columns * rows) as usize);

    for y in 0 .. rows {
      for x in 0 .. columns {
        let position = cell_center(columns, rows, IVec2::new(x, y));
        blocked.push(
          !inside_playable_area(position)
            || point_overlaps_any_obstacle(position, &obstacles, ACTOR_RADIUS),
        );
      }
    }

    Self {
      obstacles,
      blocked,
      columns,
      rows,
    }
  }

  pub(crate) fn try_move(&self, from: Vec2, delta: Vec2) -> Vec2 {
    let from = clamp_to_playable_area(from);
    let target = clamp_to_playable_area(from + delta);
    if self.segment_is_walkable(from, target) {
      return target;
    }

    let mut moved = from;
    if delta.x != 0.0 {
      let x_target = Vec2::new(target.x, moved.y);
      if self.segment_is_walkable(moved, x_target) {
        moved = x_target;
      }
    }

    if delta.y != 0.0 {
      let y_target = Vec2::new(moved.x, target.y);
      if self.segment_is_walkable(moved, y_target) {
        moved = y_target;
      }
    }

    moved
  }

  pub(crate) fn next_waypoint(&self, from: Vec2, goal: Vec2) -> Vec2 {
    let from = clamp_to_playable_area(from);
    let goal = clamp_to_playable_area(goal);

    if self.segment_is_walkable(from, goal) {
      return goal;
    }

    let Some(start) = self.walkable_cell_near(from) else {
      return from;
    };
    let Some(goal) = self.walkable_cell_near(goal) else {
      return from;
    };
    let Some(next_cell) = self.next_cell_on_path(start, goal) else {
      return from;
    };

    self.cell_center(next_cell)
  }

  pub(crate) fn segment_is_walkable(&self, from: Vec2, to: Vec2) -> bool {
    let distance = from.distance(to);
    let steps = (distance / (CELL_SIZE * 0.5)).ceil().max(1.0) as usize;

    (0 ..= steps).all(|step| {
      let amount = step as f32 / steps as f32;
      self.is_walkable(from.lerp(to, amount))
    })
  }

  fn is_walkable(&self, position: Vec2) -> bool {
    inside_playable_area(position)
      && !point_overlaps_any_obstacle(position, &self.obstacles, ACTOR_RADIUS)
  }

  fn walkable_cell_near(&self, position: Vec2) -> Option<IVec2> {
    let origin = self.world_to_cell(position)?;
    if self.cell_is_walkable(origin) {
      return Some(origin);
    }

    let mut visited = vec![false; self.blocked.len()];
    let mut queue = VecDeque::from([origin]);
    visited[self.index(origin)?] = true;

    while let Some(cell) = queue.pop_front() {
      for neighbor in self.neighbors(cell) {
        let Some(index) = self.index(neighbor) else {
          continue;
        };
        if visited[index] {
          continue;
        }
        if self.cell_is_walkable(neighbor) {
          return Some(neighbor);
        }

        visited[index] = true;
        queue.push_back(neighbor);
      }
    }

    None
  }

  fn next_cell_on_path(&self, start: IVec2, goal: IVec2) -> Option<IVec2> {
    if start == goal {
      return Some(goal);
    }

    let mut parents = vec![None; self.blocked.len()];
    let mut visited = vec![false; self.blocked.len()];
    let mut queue = VecDeque::from([start]);
    visited[self.index(start)?] = true;

    while let Some(cell) = queue.pop_front() {
      if cell == goal {
        break;
      }

      for neighbor in self.neighbors(cell) {
        let Some(index) = self.index(neighbor) else {
          continue;
        };
        if visited[index] || !self.cell_is_walkable(neighbor) {
          continue;
        }

        visited[index] = true;
        parents[index] = Some(cell);
        queue.push_back(neighbor);
      }
    }

    if !visited[self.index(goal)?] {
      return None;
    }

    let mut current = goal;
    while let Some(parent) = parents[self.index(current)?] {
      if parent == start {
        return Some(current);
      }
      current = parent;
    }

    Some(goal)
  }

  fn neighbors(&self, cell: IVec2) -> impl Iterator<Item = IVec2> {
    [
      IVec2::new(cell.x + 1, cell.y),
      IVec2::new(cell.x - 1, cell.y),
      IVec2::new(cell.x, cell.y + 1),
      IVec2::new(cell.x, cell.y - 1),
    ]
    .into_iter()
    .filter(|neighbor| self.index(*neighbor).is_some())
  }

  fn cell_is_walkable(&self, cell: IVec2) -> bool {
    self.index(cell).is_some_and(|index| !self.blocked[index])
  }

  fn world_to_cell(&self, position: Vec2) -> Option<IVec2> {
    if !inside_playable_area(position) {
      return None;
    }

    let min = -ARENA_HALF_SIZE;
    let local = position - min;
    let cell = IVec2::new(
      ((local.x / CELL_SIZE).floor() as i32).clamp(0, self.columns - 1),
      ((local.y / CELL_SIZE).floor() as i32).clamp(0, self.rows - 1),
    );

    self.index(cell).map(|_| cell)
  }

  fn cell_center(&self, cell: IVec2) -> Vec2 {
    cell_center(self.columns, self.rows, cell)
  }

  fn index(&self, cell: IVec2) -> Option<usize> {
    if cell.x < 0 || cell.x >= self.columns || cell.y < 0 || cell.y >= self.rows {
      return None;
    }

    Some((cell.y * self.columns + cell.x) as usize)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerrainSpec {
  pub(crate) center: Vec2,
  pub(crate) size: Vec2,
  pub(crate) shape: ObstacleShape,
}

impl TerrainSpec {
  fn contains(&self, position: Vec2, padding: f32) -> bool {
    let half_size = Vec2::new(
      (self.size.x * 0.5 + padding).max(1.0),
      (self.size.y * 0.5 + padding).max(1.0),
    );
    let delta = position - self.center;
    let abs_delta = delta.abs();

    match self.shape {
      ObstacleShape::Rectangle => abs_delta.x <= half_size.x && abs_delta.y <= half_size.y,
      ObstacleShape::Diamond => abs_delta.x / half_size.x + abs_delta.y / half_size.y <= 1.0,
      ObstacleShape::Ellipse => {
        let x = abs_delta.x / half_size.x;
        let y = abs_delta.y / half_size.y;
        x * x + y * y <= 1.0
      }
      ObstacleShape::Cross => {
        let bar = (half_size.x.min(half_size.y) * 0.35 + padding).max(8.0);
        (abs_delta.x <= half_size.x && abs_delta.y <= bar)
          || (abs_delta.y <= half_size.y && abs_delta.x <= bar)
      }
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MonsterSpawn {
  pub(crate) position: Vec2,
  pub(crate) red: i32,
  pub(crate) blue: i32,
  pub(crate) speed: f32,
}

pub(crate) fn map_state(map: &LevelMap) -> MapState {
  MapState {
    name: map.name.to_string(),
    half_width: ARENA_HALF_SIZE.x,
    half_height: ARENA_HALF_SIZE.y,
    obstacles: map
      .obstacles
      .iter()
      .map(|obstacle| ObstacleState {
        x: obstacle.center.x,
        y: obstacle.center.y,
        width: obstacle.size.x,
        height: obstacle.size.y,
        shape: obstacle.shape as i32,
      })
      .collect(),
  }
}

pub(crate) fn clamp_to_playable_area(position: Vec2) -> Vec2 {
  let half_size = playable_half_size();
  Vec2::new(
    position.x.clamp(-half_size.x, half_size.x),
    position.y.clamp(-half_size.y, half_size.y),
  )
}

fn playable_half_size() -> Vec2 {
  ARENA_HALF_SIZE - Vec2::splat(PLAYABLE_PADDING)
}

fn inside_playable_area(position: Vec2) -> bool {
  let half_size = playable_half_size();
  position.x >= -half_size.x
    && position.x <= half_size.x
    && position.y >= -half_size.y
    && position.y <= half_size.y
}

fn point_overlaps_any_obstacle(position: Vec2, obstacles: &[TerrainSpec], padding: f32) -> bool {
  obstacles
    .iter()
    .any(|obstacle| obstacle.contains(position, padding))
}

fn cell_center(columns: i32, rows: i32, cell: IVec2) -> Vec2 {
  let min = -ARENA_HALF_SIZE;
  let x = min.x + (cell.x as f32 + 0.5) * CELL_SIZE;
  let y = min.y + (cell.y as f32 + 0.5) * CELL_SIZE;

  clamp_to_playable_area(Vec2::new(
    x.min(min.x + columns as f32 * CELL_SIZE),
    y.min(min.y + rows as f32 * CELL_SIZE),
  ))
}

const fn roadblock(center: Vec2, size: Vec2, shape: ObstacleShape) -> TerrainSpec {
  TerrainSpec {
    center,
    size,
    shape,
  }
}

const CROSSROADS_OBSTACLES: [TerrainSpec; 4] = [
  roadblock(
    Vec2::new(-255.0, -170.0),
    Vec2::new(150.0, 70.0),
    ObstacleShape::Rectangle,
  ),
  roadblock(
    Vec2::new(240.0, 160.0),
    Vec2::new(120.0, 120.0),
    ObstacleShape::Ellipse,
  ),
  roadblock(
    Vec2::new(-90.0, 115.0),
    Vec2::new(130.0, 90.0),
    ObstacleShape::Diamond,
  ),
  roadblock(
    Vec2::new(150.0, -95.0),
    Vec2::new(110.0, 120.0),
    ObstacleShape::Cross,
  ),
];

const CROSSROADS_MONSTERS: [MonsterSpawn; 4] = [
  MonsterSpawn {
    position: Vec2::new(-320.0, 190.0),
    red: 9,
    blue: 56,
    speed: 92.0,
  },
  MonsterSpawn {
    position: Vec2::new(320.0, 190.0),
    red: 10,
    blue: 56,
    speed: 102.0,
  },
  MonsterSpawn {
    position: Vec2::new(-300.0, -190.0),
    red: 11,
    blue: 56,
    speed: 112.0,
  },
  MonsterSpawn {
    position: Vec2::new(310.0, -170.0),
    red: 12,
    blue: 56,
    speed: 122.0,
  },
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn movement_into_obstacle_is_blocked() {
    let map = LevelMap::default();
    let obs = map.obstacles[0];
    let terrain = TerrainMap::for_level(&map);
    let start = clamp_to_playable_area(Vec2::new(
      obs.center.x - obs.size.x * 0.5 - ACTOR_RADIUS - 2.0,
      obs.center.y,
    ));
    let movement = Vec2::new(obs.size.x, 0.0);

    assert_eq!(terrain.try_move(start, movement), start);
  }

  #[test]
  fn blocked_segment_routes_to_next_walkable_cell() {
    let map = LevelMap::default();
    let obs = map.obstacles[0];
    let terrain = TerrainMap::for_level(&map);
    let start = clamp_to_playable_area(Vec2::new(obs.center.x - obs.size.x, obs.center.y));
    let goal = clamp_to_playable_area(Vec2::new(obs.center.x + obs.size.x, obs.center.y));

    assert!(!terrain.segment_is_walkable(start, goal));

    let waypoint = terrain.next_waypoint(start, goal);

    assert_ne!(waypoint, start);
    assert_ne!(waypoint, goal);
    assert!(terrain.segment_is_walkable(start, waypoint));
  }
}
