use std::{collections::VecDeque, sync::Arc};

use bevy::prelude::*;
use bevy_voxel_world::prelude::{
  ChunkDespawnStrategy, ChunkSpawnStrategy, TextureIndexMapperFn, VoxelLookupDelegate,
  VoxelWorldConfig, WorldVoxel,
};

use crate::config::{ARENA_HALF_SIZE, clamp_to_arena};

const CELL_SIZE: f32 = 40.0;
const ACTOR_RADIUS: f32 = 18.0;
const VOXEL_WORLD_SCALE: f32 = 20.0;
const BOUNDARY_WALL_THICKNESS: f32 = VOXEL_WORLD_SCALE;
const PLAYABLE_PADDING: f32 = ACTOR_RADIUS + BOUNDARY_WALL_THICKNESS;
const VOXEL_OBSTACLE_HEIGHT: i32 = 3;
const GROUND_MATERIAL: u8 = 0;
const WALL_MATERIAL: u8 = 1;
const OBSTACLE_MATERIAL: u8 = 2;
const TERRAIN_SPECS: [TerrainSpec; 4] = [
  TerrainSpec {
    center: Vec2::new(-140.0, 20.0),
    size: Vec2::new(90.0, 260.0),
  },
  TerrainSpec {
    center: Vec2::new(170.0, 110.0),
    size: Vec2::new(190.0, 70.0),
  },
  TerrainSpec {
    center: Vec2::new(120.0, -140.0),
    size: Vec2::new(150.0, 80.0),
  },
  TerrainSpec {
    center: Vec2::new(-330.0, -20.0),
    size: Vec2::new(70.0, 150.0),
  },
];

#[derive(Resource)]
pub(crate) struct TerrainMap {
  obstacles: Vec<TerrainSpec>,
  blocked: Vec<bool>,
  columns: i32,
  rows: i32,
}

#[derive(Debug, Clone, Copy)]
struct TerrainSpec {
  center: Vec2,
  size: Vec2,
}

#[derive(Resource, Clone, Default)]
pub(crate) struct GameVoxelWorld;

impl VoxelWorldConfig for GameVoxelWorld {
  type MaterialIndex = u8;
  type ChunkUserBundle = ();

  fn spawning_distance(&self) -> u32 {
    16
  }

  fn min_despawn_distance(&self) -> u32 {
    1
  }

  fn spawning_rays(&self) -> usize {
    400
  }

  fn spawning_ray_margin(&self) -> u32 {
    80
  }

  fn chunk_despawn_strategy(&self) -> ChunkDespawnStrategy {
    // Only despawn when truly far; never cull by frustum so the fixed
    // overhead camera never creates black corners.
    ChunkDespawnStrategy::FarAway
  }

  fn chunk_spawn_strategy(&self) -> ChunkSpawnStrategy {
    // Flood-fill outward from the camera instead of relying on the frustum
    // corner test, which fails for steeply-angled cameras where far ground
    // chunks have all 8 AABB corners outside the view frustum.
    ChunkSpawnStrategy::Close
  }

  fn attach_chunks_to_root(&self) -> bool {
    false
  }

  fn texture_index_mapper(&self) -> TextureIndexMapperFn<Self::MaterialIndex> {
    Arc::new(|material| match material {
      GROUND_MATERIAL => [1, 1, 2],
      WALL_MATERIAL => [2, 2, 2],
      OBSTACLE_MATERIAL => [3, 3, 3],
      _ => [0, 0, 0],
    })
  }

  fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
    Box::new(|_, _, _| Box::new(|position, _| voxel_at(position)))
  }
}

pub(crate) fn game_to_world_position(position: Vec2, height: f32) -> Vec3 {
  Vec3::new(
    position.x / VOXEL_WORLD_SCALE,
    height,
    position.y / VOXEL_WORLD_SCALE,
  )
}

impl Default for TerrainMap {
  fn default() -> Self {
    Self::new()
  }
}

impl TerrainMap {
  pub(crate) fn new() -> Self {
    let columns = ((ARENA_HALF_SIZE.x * 2.0) / CELL_SIZE).ceil() as i32;
    let rows = ((ARENA_HALF_SIZE.y * 2.0) / CELL_SIZE).ceil() as i32;
    let obstacles = TERRAIN_SPECS.to_vec();
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

  fn is_walkable(&self, position: Vec2) -> bool {
    inside_playable_area(position)
      && !point_overlaps_any_obstacle(position, &self.obstacles, ACTOR_RADIUS)
  }

  pub(crate) fn segment_is_walkable(&self, from: Vec2, to: Vec2) -> bool {
    let distance = from.distance(to);
    let steps = (distance / (CELL_SIZE * 0.5)).ceil().max(1.0) as usize;

    (0 ..= steps).all(|step| {
      let amount = step as f32 / steps as f32;
      self.is_walkable(from.lerp(to, amount))
    })
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

fn cell_center(columns: i32, rows: i32, cell: IVec2) -> Vec2 {
  let min = -ARENA_HALF_SIZE;
  let x = min.x + (cell.x as f32 + 0.5) * CELL_SIZE;
  let y = min.y + (cell.y as f32 + 0.5) * CELL_SIZE;

  clamp_to_arena(Vec2::new(
    x.min(min.x + columns as f32 * CELL_SIZE),
    y.min(min.y + rows as f32 * CELL_SIZE),
  ))
}

pub(crate) fn clamp_to_playable_area(position: Vec2) -> Vec2 {
  let half_size = playable_half_size();
  Vec2::new(
    position.x.clamp(-half_size.x, half_size.x),
    position.y.clamp(-half_size.y, half_size.y),
  )
}

fn point_overlaps_any_obstacle(position: Vec2, obstacles: &[TerrainSpec], padding: f32) -> bool {
  obstacles
    .iter()
    .any(|obstacle| obstacle.contains(position, padding))
}

fn voxel_at(position: IVec3) -> WorldVoxel<u8> {
  let game_position = voxel_to_game_center(position);
  if position.y == -1 {
    return WorldVoxel::Solid(GROUND_MATERIAL);
  }

  if !inside_arena(game_position) {
    return WorldVoxel::Air;
  }

  if (0 .. VOXEL_OBSTACLE_HEIGHT).contains(&position.y) {
    if is_boundary_wall(game_position) {
      return WorldVoxel::Solid(WALL_MATERIAL);
    }
    if point_overlaps_any_obstacle(game_position, &TERRAIN_SPECS, 0.0) {
      return WorldVoxel::Solid(OBSTACLE_MATERIAL);
    }
  }

  WorldVoxel::Air
}

fn voxel_to_game_center(position: IVec3) -> Vec2 {
  Vec2::new(
    (position.x as f32 + 0.5) * VOXEL_WORLD_SCALE,
    (position.z as f32 + 0.5) * VOXEL_WORLD_SCALE,
  )
}

fn inside_arena(position: Vec2) -> bool {
  position.x >= -ARENA_HALF_SIZE.x
    && position.x <= ARENA_HALF_SIZE.x
    && position.y >= -ARENA_HALF_SIZE.y
    && position.y <= ARENA_HALF_SIZE.y
}

fn inside_playable_area(position: Vec2) -> bool {
  let half_size = playable_half_size();
  position.x >= -half_size.x
    && position.x <= half_size.x
    && position.y >= -half_size.y
    && position.y <= half_size.y
}

fn playable_half_size() -> Vec2 {
  ARENA_HALF_SIZE - Vec2::splat(PLAYABLE_PADDING)
}

fn is_boundary_wall(position: Vec2) -> bool {
  let margin = BOUNDARY_WALL_THICKNESS * 0.5;
  position.x <= -ARENA_HALF_SIZE.x + margin
    || position.x >= ARENA_HALF_SIZE.x - margin
    || position.y <= -ARENA_HALF_SIZE.y + margin
    || position.y >= ARENA_HALF_SIZE.y - margin
}

impl TerrainSpec {
  fn contains(self, position: Vec2, padding: f32) -> bool {
    let half_size = self.size * 0.5 + Vec2::splat(padding);
    let delta = position - self.center;

    delta.x.abs() <= half_size.x && delta.y.abs() <= half_size.y
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clear_segment_uses_goal_as_waypoint() {
    let terrain = TerrainMap::new();
    let start = Vec2::new(-360.0, 200.0);
    let goal = Vec2::new(-280.0, 200.0);

    assert!(terrain.segment_is_walkable(start, goal));
    assert_eq!(terrain.next_waypoint(start, goal), goal);
  }

  #[test]
  fn movement_into_obstacle_is_blocked() {
    let terrain = TerrainMap::new();
    let start = Vec2::new(-260.0, 20.0);
    let movement = Vec2::new(210.0, 0.0);

    assert_eq!(terrain.try_move(start, movement), start);
  }

  #[test]
  fn movement_past_boundary_stays_inside_playable_area() {
    let terrain = TerrainMap::new();
    let start = Vec2::new(playable_half_size().x - 2.0, 0.0);
    let movement = Vec2::new(120.0, 0.0);

    let moved = terrain.try_move(start, movement);

    assert!(inside_playable_area(moved));
    assert_eq!(moved, Vec2::new(playable_half_size().x, 0.0));
  }

  #[test]
  fn boundary_wall_is_not_walkable() {
    let terrain = TerrainMap::new();

    assert!(!terrain.segment_is_walkable(
      Vec2::new(playable_half_size().x + 1.0, 0.0),
      Vec2::new(ARENA_HALF_SIZE.x, 0.0)
    ));
  }

  #[test]
  fn blocked_segment_routes_to_next_walkable_cell() {
    let terrain = TerrainMap::new();
    let start = Vec2::new(-260.0, 20.0);
    let goal = Vec2::new(10.0, 20.0);

    assert!(!terrain.segment_is_walkable(start, goal));

    let waypoint = terrain.next_waypoint(start, goal);

    assert_ne!(waypoint, start);
    assert_ne!(waypoint, goal);
    assert!(terrain.segment_is_walkable(start, waypoint));
  }

  #[test]
  fn voxel_lookup_builds_ground_and_obstacle_columns() {
    assert_eq!(
      voxel_at(IVec3::new(0, -1, 0)),
      WorldVoxel::Solid(GROUND_MATERIAL)
    );
    assert_eq!(
      voxel_at(IVec3::new(-7, 0, 1)),
      WorldVoxel::Solid(OBSTACLE_MATERIAL)
    );
    assert_eq!(
      voxel_at(IVec3::new(-7, VOXEL_OBSTACLE_HEIGHT, 1)),
      WorldVoxel::Air
    );
  }

  #[test]
  fn visual_ground_extends_beyond_arena_to_fill_view() {
    let outside_arena = IVec3::new(80, -1, 80);

    assert_eq!(voxel_at(outside_arena), WorldVoxel::Solid(GROUND_MATERIAL));
    assert_eq!(voxel_at(outside_arena + IVec3::Y), WorldVoxel::Air);
  }
}
