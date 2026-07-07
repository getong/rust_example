use std::{
  collections::VecDeque,
  sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
  },
};

use bevy::prelude::*;
use bevy_voxel_world::prelude::{
  ChunkDespawnStrategy, ChunkSpawnStrategy, TextureIndexMapperFn, VoxelLookupDelegate, VoxelWorld,
  VoxelWorldConfig, WorldVoxel,
};

use crate::config::{ARENA_HALF_SIZE, clamp_to_arena};

const CELL_SIZE: f32 = 40.0;
const ACTOR_RADIUS: f32 = 18.0;
const VOXEL_WORLD_SCALE: f32 = 20.0;
const BOUNDARY_WALL_THICKNESS: f32 = VOXEL_WORLD_SCALE;
const PLAYABLE_PADDING: f32 = ACTOR_RADIUS + BOUNDARY_WALL_THICKNESS;
const VISUAL_GROUND_PADDING: f32 = VOXEL_WORLD_SCALE * 8.0;
const MAX_VOXEL_TERRAIN_HEIGHT: i32 = 6;
const CROSSROADS_GROUND_MATERIAL: u8 = 0;
const CROSSROADS_PATH_MATERIAL: u8 = 1;
const CROSSROADS_WALL_MATERIAL: u8 = 2;
const CROSSROADS_OBSTACLE_MATERIAL: u8 = 3;
const SWITCHBACK_GROUND_MATERIAL: u8 = 4;
const SWITCHBACK_PATH_MATERIAL: u8 = 5;
const SWITCHBACK_WALL_MATERIAL: u8 = 6;
const SWITCHBACK_OBSTACLE_MATERIAL: u8 = 7;
const CITADEL_GROUND_MATERIAL: u8 = 8;
const CITADEL_PATH_MATERIAL: u8 = 9;
const CITADEL_WALL_MATERIAL: u8 = 10;
const CITADEL_OBSTACLE_MATERIAL: u8 = 11;
const MAZE_GROUND_MATERIAL: u8 = 12;
const MAZE_PATH_MATERIAL: u8 = 13;
const MAZE_WALL_MATERIAL: u8 = 14;
const MAZE_OBSTACLE_MATERIAL: u8 = 15;
const AMPHITHEATER_GROUND_MATERIAL: u8 = 16;
const AMPHITHEATER_PATH_MATERIAL: u8 = 17;
const AMPHITHEATER_WALL_MATERIAL: u8 = 18;
const AMPHITHEATER_OBSTACLE_MATERIAL: u8 = 19;
const RAVINE_GROUND_MATERIAL: u8 = 20;
const RAVINE_PATH_MATERIAL: u8 = 21;
const RAVINE_WALL_MATERIAL: u8 = 22;
const RAVINE_OBSTACLE_MATERIAL: u8 = 23;
const DEFAULT_LEVEL_INDEX: u8 = 0;

static VISUAL_LEVEL_INDEX: AtomicU8 = AtomicU8::new(DEFAULT_LEVEL_INDEX);

const fn roadblock(center: Vec2, size: Vec2, shape: ObstacleShape, material: u8) -> TerrainSpec {
  TerrainSpec {
    center,
    size,
    shape,
    material,
  }
}

const CROSSROADS_OBSTACLES: [TerrainSpec; 4] = [
  roadblock(
    Vec2::new(-255.0, -170.0),
    Vec2::new(150.0, 70.0),
    ObstacleShape::Rectangle,
    CROSSROADS_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(240.0, 160.0),
    Vec2::new(120.0, 120.0),
    ObstacleShape::Ellipse,
    CROSSROADS_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-90.0, 115.0),
    Vec2::new(130.0, 90.0),
    ObstacleShape::Diamond,
    CROSSROADS_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(150.0, -95.0),
    Vec2::new(110.0, 120.0),
    ObstacleShape::Cross,
    CROSSROADS_OBSTACLE_MATERIAL,
  ),
];

const SWITCHBACK_OBSTACLES: [TerrainSpec; 5] = [
  roadblock(
    Vec2::new(-315.0, -215.0),
    Vec2::new(110.0, 85.0),
    ObstacleShape::Ellipse,
    SWITCHBACK_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(-220.0, 80.0),
    Vec2::new(95.0, 140.0),
    ObstacleShape::Diamond,
    SWITCHBACK_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-35.0, -160.0),
    Vec2::new(145.0, 95.0),
    ObstacleShape::Cross,
    SWITCHBACK_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(175.0, 70.0),
    Vec2::new(155.0, 65.0),
    ObstacleShape::Rectangle,
    SWITCHBACK_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(315.0, -110.0),
    Vec2::new(105.0, 125.0),
    ObstacleShape::Ellipse,
    SWITCHBACK_WALL_MATERIAL,
  ),
];

const CITADEL_OBSTACLES: [TerrainSpec; 6] = [
  roadblock(
    Vec2::new(-300.0, -45.0),
    Vec2::new(120.0, 95.0),
    ObstacleShape::Diamond,
    CITADEL_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(300.0, 35.0),
    Vec2::new(130.0, 80.0),
    ObstacleShape::Cross,
    CITADEL_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-155.0, -210.0),
    Vec2::new(110.0, 85.0),
    ObstacleShape::Rectangle,
    CITADEL_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(150.0, 210.0),
    Vec2::new(115.0, 115.0),
    ObstacleShape::Ellipse,
    CITADEL_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-55.0, 95.0),
    Vec2::new(120.0, 75.0),
    ObstacleShape::Diamond,
    CITADEL_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(85.0, -100.0),
    Vec2::new(100.0, 120.0),
    ObstacleShape::Cross,
    CITADEL_OBSTACLE_MATERIAL,
  ),
];

const MAZE_OBSTACLES: [TerrainSpec; 7] = [
  roadblock(
    Vec2::new(-325.0, -220.0),
    Vec2::new(115.0, 70.0),
    ObstacleShape::Cross,
    MAZE_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(-225.0, -35.0),
    Vec2::new(85.0, 150.0),
    ObstacleShape::Rectangle,
    MAZE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-90.0, -225.0),
    Vec2::new(140.0, 75.0),
    ObstacleShape::Ellipse,
    MAZE_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(130.0, -75.0),
    Vec2::new(120.0, 115.0),
    ObstacleShape::Diamond,
    MAZE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(210.0, -210.0),
    Vec2::new(105.0, 90.0),
    ObstacleShape::Cross,
    MAZE_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(280.0, 45.0),
    Vec2::new(95.0, 135.0),
    ObstacleShape::Rectangle,
    MAZE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-25.0, 210.0),
    Vec2::new(155.0, 70.0),
    ObstacleShape::Ellipse,
    MAZE_PATH_MATERIAL,
  ),
];

const AMPHITHEATER_OBSTACLES: [TerrainSpec; 8] = [
  roadblock(
    Vec2::new(-305.0, 185.0),
    Vec2::new(110.0, 95.0),
    ObstacleShape::Rectangle,
    AMPHITHEATER_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(-250.0, -160.0),
    Vec2::new(115.0, 120.0),
    ObstacleShape::Ellipse,
    AMPHITHEATER_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-125.0, 45.0),
    Vec2::new(100.0, 80.0),
    ObstacleShape::Diamond,
    AMPHITHEATER_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(0.0, -215.0),
    Vec2::new(150.0, 75.0),
    ObstacleShape::Cross,
    AMPHITHEATER_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(140.0, 180.0),
    Vec2::new(105.0, 110.0),
    ObstacleShape::Rectangle,
    AMPHITHEATER_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(260.0, -50.0),
    Vec2::new(115.0, 90.0),
    ObstacleShape::Ellipse,
    AMPHITHEATER_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(305.0, 130.0),
    Vec2::new(95.0, 115.0),
    ObstacleShape::Diamond,
    AMPHITHEATER_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(115.0, 95.0),
    Vec2::new(95.0, 100.0),
    ObstacleShape::Cross,
    AMPHITHEATER_OBSTACLE_MATERIAL,
  ),
];

const RAVINE_OBSTACLES: [TerrainSpec; 9] = [
  roadblock(
    Vec2::new(-305.0, -220.0),
    Vec2::new(115.0, 80.0),
    ObstacleShape::Ellipse,
    RAVINE_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(-185.0, 40.0),
    Vec2::new(100.0, 130.0),
    ObstacleShape::Diamond,
    RAVINE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(-225.0, 215.0),
    Vec2::new(125.0, 80.0),
    ObstacleShape::Cross,
    RAVINE_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(-85.0, -140.0),
    Vec2::new(105.0, 120.0),
    ObstacleShape::Rectangle,
    RAVINE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(45.0, 215.0),
    Vec2::new(120.0, 75.0),
    ObstacleShape::Ellipse,
    RAVINE_WALL_MATERIAL,
  ),
  roadblock(
    Vec2::new(165.0, -220.0),
    Vec2::new(95.0, 115.0),
    ObstacleShape::Diamond,
    RAVINE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(250.0, 90.0),
    Vec2::new(130.0, 75.0),
    ObstacleShape::Cross,
    RAVINE_PATH_MATERIAL,
  ),
  roadblock(
    Vec2::new(310.0, -75.0),
    Vec2::new(100.0, 105.0),
    ObstacleShape::Rectangle,
    RAVINE_OBSTACLE_MATERIAL,
  ),
  roadblock(
    Vec2::new(0.0, 235.0),
    Vec2::new(145.0, 70.0),
    ObstacleShape::Ellipse,
    RAVINE_WALL_MATERIAL,
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

const SWITCHBACK_MONSTERS: [MonsterSpawn; 5] = [
  MonsterSpawn {
    position: Vec2::new(-340.0, -210.0),
    red: 11,
    blue: 64,
    speed: 105.0,
  },
  MonsterSpawn {
    position: Vec2::new(-210.0, 210.0),
    red: 12,
    blue: 64,
    speed: 110.0,
  },
  MonsterSpawn {
    position: Vec2::new(40.0, -220.0),
    red: 13,
    blue: 68,
    speed: 118.0,
  },
  MonsterSpawn {
    position: Vec2::new(240.0, 210.0),
    red: 14,
    blue: 68,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(340.0, -200.0),
    red: 15,
    blue: 72,
    speed: 130.0,
  },
];

const CITADEL_MONSTERS: [MonsterSpawn; 6] = [
  MonsterSpawn {
    position: Vec2::new(-340.0, 230.0),
    red: 13,
    blue: 74,
    speed: 114.0,
  },
  MonsterSpawn {
    position: Vec2::new(340.0, 230.0),
    red: 14,
    blue: 74,
    speed: 118.0,
  },
  MonsterSpawn {
    position: Vec2::new(-340.0, -230.0),
    red: 15,
    blue: 80,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(340.0, -230.0),
    red: 16,
    blue: 80,
    speed: 128.0,
  },
  MonsterSpawn {
    position: Vec2::new(-20.0, 150.0),
    red: 17,
    blue: 88,
    speed: 132.0,
  },
  MonsterSpawn {
    position: Vec2::new(20.0, -150.0),
    red: 18,
    blue: 88,
    speed: 136.0,
  },
];

const MAZE_MONSTERS: [MonsterSpawn; 6] = [
  MonsterSpawn {
    position: Vec2::new(-280.0, -240.0),
    red: 13,
    blue: 70,
    speed: 105.0,
  },
  MonsterSpawn {
    position: Vec2::new(-60.0, -220.0),
    red: 14,
    blue: 74,
    speed: 112.0,
  },
  MonsterSpawn {
    position: Vec2::new(310.0, -160.0),
    red: 15,
    blue: 78,
    speed: 118.0,
  },
  MonsterSpawn {
    position: Vec2::new(-310.0, 100.0),
    red: 16,
    blue: 82,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(50.0, 200.0),
    red: 17,
    blue: 86,
    speed: 128.0,
  },
  MonsterSpawn {
    position: Vec2::new(280.0, 240.0),
    red: 18,
    blue: 90,
    speed: 134.0,
  },
];

const AMPHITHEATER_MONSTERS: [MonsterSpawn; 7] = [
  MonsterSpawn {
    position: Vec2::new(-340.0, -240.0),
    red: 15,
    blue: 78,
    speed: 112.0,
  },
  MonsterSpawn {
    position: Vec2::new(340.0, -240.0),
    red: 16,
    blue: 82,
    speed: 118.0,
  },
  MonsterSpawn {
    position: Vec2::new(-340.0, 240.0),
    red: 17,
    blue: 86,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(340.0, 240.0),
    red: 17,
    blue: 86,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(-160.0, 0.0),
    red: 18,
    blue: 92,
    speed: 130.0,
  },
  MonsterSpawn {
    position: Vec2::new(160.0, 0.0),
    red: 19,
    blue: 96,
    speed: 134.0,
  },
  MonsterSpawn {
    position: Vec2::new(0.0, -200.0),
    red: 20,
    blue: 104,
    speed: 140.0,
  },
];

const RAVINE_MONSTERS: [MonsterSpawn; 7] = [
  MonsterSpawn {
    position: Vec2::new(-280.0, -240.0),
    red: 16,
    blue: 82,
    speed: 118.0,
  },
  MonsterSpawn {
    position: Vec2::new(-280.0, 0.0),
    red: 17,
    blue: 88,
    speed: 124.0,
  },
  MonsterSpawn {
    position: Vec2::new(-280.0, 240.0),
    red: 18,
    blue: 92,
    speed: 128.0,
  },
  MonsterSpawn {
    position: Vec2::new(280.0, -240.0),
    red: 18,
    blue: 94,
    speed: 128.0,
  },
  MonsterSpawn {
    position: Vec2::new(280.0, 0.0),
    red: 19,
    blue: 98,
    speed: 132.0,
  },
  MonsterSpawn {
    position: Vec2::new(280.0, 240.0),
    red: 20,
    blue: 102,
    speed: 136.0,
  },
  MonsterSpawn {
    position: Vec2::new(0.0, 0.0),
    red: 24,
    blue: 130,
    speed: 148.0,
  },
];

const LEVEL_MAP_TEMPLATES: [LevelMapTemplate; 6] = [
  LevelMapTemplate {
    index: 0,
    name: "Crossroads",
    hint: "open lanes",
    player_spawn: Vec2::ZERO,
    obstacles: &CROSSROADS_OBSTACLES,
    monsters: &CROSSROADS_MONSTERS,
    ground_material: CROSSROADS_GROUND_MATERIAL,
    path_material: CROSSROADS_PATH_MATERIAL,
    wall_material: CROSSROADS_WALL_MATERIAL,
    obstacle_height: 3,
    pattern: TerrainPattern::Crossroads,
  },
  LevelMapTemplate {
    index: 1,
    name: "Switchback",
    hint: "zig-zag cover",
    player_spawn: Vec2::new(-340.0, 0.0),
    obstacles: &SWITCHBACK_OBSTACLES,
    monsters: &SWITCHBACK_MONSTERS,
    ground_material: SWITCHBACK_GROUND_MATERIAL,
    path_material: SWITCHBACK_PATH_MATERIAL,
    wall_material: SWITCHBACK_WALL_MATERIAL,
    obstacle_height: 4,
    pattern: TerrainPattern::Switchback,
  },
  LevelMapTemplate {
    index: 2,
    name: "Citadel",
    hint: "inner keep",
    player_spawn: Vec2::new(0.0, -245.0),
    obstacles: &CITADEL_OBSTACLES,
    monsters: &CITADEL_MONSTERS,
    ground_material: CITADEL_GROUND_MATERIAL,
    path_material: CITADEL_PATH_MATERIAL,
    wall_material: CITADEL_WALL_MATERIAL,
    obstacle_height: 5,
    pattern: TerrainPattern::Citadel,
  },
  LevelMapTemplate {
    index: 3,
    name: "Maze",
    hint: "twist and turn",
    player_spawn: Vec2::new(0.0, 0.0),
    obstacles: &MAZE_OBSTACLES,
    monsters: &MAZE_MONSTERS,
    ground_material: MAZE_GROUND_MATERIAL,
    path_material: MAZE_PATH_MATERIAL,
    wall_material: MAZE_WALL_MATERIAL,
    obstacle_height: 3,
    pattern: TerrainPattern::Maze,
  },
  LevelMapTemplate {
    index: 4,
    name: "Amphitheater",
    hint: "ring of battle",
    player_spawn: Vec2::new(0.0, 0.0),
    obstacles: &AMPHITHEATER_OBSTACLES,
    monsters: &AMPHITHEATER_MONSTERS,
    ground_material: AMPHITHEATER_GROUND_MATERIAL,
    path_material: AMPHITHEATER_PATH_MATERIAL,
    wall_material: AMPHITHEATER_WALL_MATERIAL,
    obstacle_height: 4,
    pattern: TerrainPattern::Amphitheater,
  },
  LevelMapTemplate {
    index: 5,
    name: "Ravine",
    hint: "split and conquer",
    player_spawn: Vec2::new(-280.0, 0.0),
    obstacles: &RAVINE_OBSTACLES,
    monsters: &RAVINE_MONSTERS,
    ground_material: RAVINE_GROUND_MATERIAL,
    path_material: RAVINE_PATH_MATERIAL,
    wall_material: RAVINE_WALL_MATERIAL,
    obstacle_height: 4,
    pattern: TerrainPattern::Ravine,
  },
];

#[derive(Resource)]
pub(crate) struct TerrainMap {
  obstacles: Vec<TerrainSpec>,
  blocked: Vec<bool>,
  columns: i32,
  rows: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerrainSpec {
  pub(crate) center: Vec2,
  pub(crate) size: Vec2,
  pub(crate) shape: ObstacleShape,
  pub(crate) material: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MonsterSpawn {
  pub(crate) position: Vec2,
  pub(crate) red: i32,
  pub(crate) blue: i32,
  pub(crate) speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObstacleShape {
  Rectangle,
  Diamond,
  Ellipse,
  Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerrainPattern {
  Crossroads,
  Switchback,
  Citadel,
  Maze,
  Amphitheater,
  Ravine,
}

#[derive(Resource, Debug, Clone)]
pub(crate) struct LevelMap {
  pub(crate) index: u8,
  pub(crate) name: &'static str,
  pub(crate) hint: &'static str,
  pub(crate) player_spawn: Vec2,
  pub(crate) obstacles: Vec<TerrainSpec>,
  pub(crate) monsters: Vec<MonsterSpawn>,
  pub(crate) ground_material: u8,
  pub(crate) path_material: u8,
  pub(crate) wall_material: u8,
  pub(crate) obstacle_height: i32,
  pub(crate) pattern: TerrainPattern,
}

#[derive(Debug, Clone, Copy)]
struct LevelMapTemplate {
  index: u8,
  name: &'static str,
  hint: &'static str,
  player_spawn: Vec2,
  obstacles: &'static [TerrainSpec],
  monsters: &'static [MonsterSpawn],
  ground_material: u8,
  path_material: u8,
  wall_material: u8,
  obstacle_height: i32,
  pattern: TerrainPattern,
}

pub(crate) fn default_level_map() -> LevelMap {
  level_map(0)
}

pub(crate) fn level_count() -> usize {
  LEVEL_MAP_TEMPLATES.len()
}

pub(crate) fn level_map(index: usize) -> LevelMap {
  let t = &LEVEL_MAP_TEMPLATES[index.min(LEVEL_MAP_TEMPLATES.len() - 1)];
  LevelMap {
    index: t.index,
    name: t.name,
    hint: t.hint,
    player_spawn: t.player_spawn,
    obstacles: t.obstacles.to_vec(),
    monsters: t.monsters.to_vec(),
    ground_material: t.ground_material,
    path_material: t.path_material,
    wall_material: t.wall_material,
    obstacle_height: t.obstacle_height,
    pattern: t.pattern,
  }
}

pub(crate) fn set_visual_level_map(map: &LevelMap) {
  VISUAL_LEVEL_INDEX.store(map.index, Ordering::Relaxed);
}

#[derive(Resource, Clone, Default)]
pub(crate) struct GameVoxelWorld;

impl VoxelWorldConfig for GameVoxelWorld {
  type MaterialIndex = u8;
  type ChunkUserBundle = ();

  fn spawning_distance(&self) -> u32 {
    3
  }

  fn min_despawn_distance(&self) -> u32 {
    2
  }

  fn spawning_rays(&self) -> usize {
    0
  }

  fn spawning_ray_margin(&self) -> u32 {
    0
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
      CROSSROADS_GROUND_MATERIAL => [1, 1, 2],
      CROSSROADS_PATH_MATERIAL => [0, 0, 1],
      CROSSROADS_WALL_MATERIAL => [2, 2, 2],
      CROSSROADS_OBSTACLE_MATERIAL => [3, 3, 3],
      SWITCHBACK_GROUND_MATERIAL => [0, 0, 1],
      SWITCHBACK_PATH_MATERIAL => [1, 1, 2],
      SWITCHBACK_WALL_MATERIAL => [3, 3, 3],
      SWITCHBACK_OBSTACLE_MATERIAL => [2, 2, 2],
      CITADEL_GROUND_MATERIAL => [2, 2, 2],
      CITADEL_PATH_MATERIAL => [3, 3, 3],
      CITADEL_WALL_MATERIAL => [1, 1, 1],
      CITADEL_OBSTACLE_MATERIAL => [0, 0, 0],
      MAZE_GROUND_MATERIAL => [2, 2, 1],
      MAZE_PATH_MATERIAL => [3, 3, 0],
      MAZE_WALL_MATERIAL => [0, 0, 0],
      MAZE_OBSTACLE_MATERIAL => [1, 1, 2],
      AMPHITHEATER_GROUND_MATERIAL => [3, 3, 3],
      AMPHITHEATER_PATH_MATERIAL => [1, 1, 2],
      AMPHITHEATER_WALL_MATERIAL => [2, 2, 2],
      AMPHITHEATER_OBSTACLE_MATERIAL => [0, 0, 0],
      RAVINE_GROUND_MATERIAL => [0, 0, 0],
      RAVINE_PATH_MATERIAL => [2, 2, 1],
      RAVINE_WALL_MATERIAL => [3, 3, 3],
      RAVINE_OBSTACLE_MATERIAL => [1, 1, 1],
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
    Self::for_level(default_level_map())
  }

  pub(crate) fn for_level(map: LevelMap) -> Self {
    let columns = ((ARENA_HALF_SIZE.x * 2.0) / CELL_SIZE).ceil() as i32;
    let rows = ((ARENA_HALF_SIZE.y * 2.0) / CELL_SIZE).ceil() as i32;
    let obstacles = map.obstacles.to_vec();
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
  obstacle_at_position(position, obstacles, padding).is_some()
}

fn obstacle_at_position(
  position: Vec2,
  obstacles: &[TerrainSpec],
  padding: f32,
) -> Option<&TerrainSpec> {
  obstacles
    .iter()
    .find(|obstacle| obstacle.contains(position, padding))
}

fn voxel_at(position: IVec3) -> WorldVoxel<u8> {
  let map = visual_level_map();
  voxel_at_for_map(&map, position)
}

fn voxel_at_for_map(map: &LevelMap, position: IVec3) -> WorldVoxel<u8> {
  let game_position = voxel_to_game_center(position);
  if position.y == -1 {
    if !inside_visual_ground(game_position) {
      return WorldVoxel::Air;
    }
    return WorldVoxel::Solid(ground_material_at(map, position, game_position));
  }

  if !inside_arena(game_position) {
    return WorldVoxel::Air;
  }

  if (0 .. map.obstacle_height).contains(&position.y) {
    if is_boundary_wall(game_position) {
      return WorldVoxel::Solid(map.wall_material);
    }
    if let Some(obstacle) = obstacle_at_position(game_position, &map.obstacles, 0.0) {
      return WorldVoxel::Solid(obstacle.material);
    }
  }

  WorldVoxel::Air
}

fn visual_level_map() -> LevelMap {
  level_map(VISUAL_LEVEL_INDEX.load(Ordering::Relaxed) as usize)
}

pub(crate) fn write_level_voxels(voxel_world: &mut VoxelWorld<GameVoxelWorld>, map: &LevelMap) {
  set_visual_level_map(map);

  let visual_half_size = visual_ground_half_size();
  let min_x = (-visual_half_size.x / VOXEL_WORLD_SCALE).floor() as i32 - 1;
  let max_x = (visual_half_size.x / VOXEL_WORLD_SCALE).ceil() as i32 + 1;
  let min_z = (-visual_half_size.y / VOXEL_WORLD_SCALE).floor() as i32 - 1;
  let max_z = (visual_half_size.y / VOXEL_WORLD_SCALE).ceil() as i32 + 1;

  for x in min_x ..= max_x {
    for z in min_z ..= max_z {
      for y in -1 .. MAX_VOXEL_TERRAIN_HEIGHT {
        let position = IVec3::new(x, y, z);
        voxel_world.set_voxel(position, voxel_at_for_map(map, position));
      }
    }
  }
}

fn voxel_to_game_center(position: IVec3) -> Vec2 {
  Vec2::new(
    (position.x as f32 + 0.5) * VOXEL_WORLD_SCALE,
    (position.z as f32 + 0.5) * VOXEL_WORLD_SCALE,
  )
}

fn ground_material_at(map: &LevelMap, voxel: IVec3, position: Vec2) -> u8 {
  if ground_path_contains(map.pattern, voxel, position) {
    map.path_material
  } else {
    map.ground_material
  }
}

fn ground_path_contains(pattern: TerrainPattern, voxel: IVec3, position: Vec2) -> bool {
  match pattern {
    TerrainPattern::Crossroads => position.x.abs() <= 70.0 || position.y.abs() <= 70.0,
    TerrainPattern::Switchback => {
      let diagonal = (voxel.x + voxel.z * 2).rem_euclid(7);
      diagonal <= 1 || (voxel.x - voxel.z).rem_euclid(11) == 0
    }
    TerrainPattern::Citadel => {
      let central_plaza = position.x.abs() <= 90.0 && position.y.abs() <= 90.0;
      let main_axis = position.x.abs() <= 45.0 || position.y.abs() <= 45.0;
      let ring = position.x.abs().max(position.y.abs());

      central_plaza || main_axis || (210.0 ..= 250.0).contains(&ring)
    }
    TerrainPattern::Maze => {
      let block_y = (position.y / 80.0).floor() as i32;
      let even_row = block_y % 2 == 0;
      if even_row {
        position.x.rem_euclid(80.0) < 28.0
      } else {
        position.y.rem_euclid(80.0) < 28.0
      }
    }
    TerrainPattern::Amphitheater => {
      let dist = position.distance(Vec2::ZERO);
      let ring = (dist / 50.0) as i32;
      ring % 2 == 0 || dist < 35.0
    }
    TerrainPattern::Ravine => {
      let center_path = position.x.abs() <= 22.0;
      let cross_path = position.y.abs().rem_euclid(120.0) < 20.0;
      center_path || cross_path
    }
  }
}

fn inside_arena(position: Vec2) -> bool {
  position.x >= -ARENA_HALF_SIZE.x
    && position.x <= ARENA_HALF_SIZE.x
    && position.y >= -ARENA_HALF_SIZE.y
    && position.y <= ARENA_HALF_SIZE.y
}

fn inside_visual_ground(position: Vec2) -> bool {
  let half_size = visual_ground_half_size();
  position.x >= -half_size.x
    && position.x <= half_size.x
    && position.y >= -half_size.y
    && position.y <= half_size.y
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

fn visual_ground_half_size() -> Vec2 {
  ARENA_HALF_SIZE + Vec2::splat(VISUAL_GROUND_PADDING)
}

fn is_boundary_wall(position: Vec2) -> bool {
  let margin = BOUNDARY_WALL_THICKNESS * 0.5;
  position.x <= -ARENA_HALF_SIZE.x + margin
    || position.x >= ARENA_HALF_SIZE.x - margin
    || position.y <= -ARENA_HALF_SIZE.y + margin
    || position.y >= ARENA_HALF_SIZE.y - margin
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

#[cfg(test)]
mod tests {
  use super::*;

  fn obstacle_center_voxel(obstacle: TerrainSpec) -> IVec3 {
    IVec3::new(
      (obstacle.center.x / VOXEL_WORLD_SCALE).floor() as i32,
      0,
      (obstacle.center.y / VOXEL_WORLD_SCALE).floor() as i32,
    )
  }

  #[test]
  fn clear_segment_uses_goal_as_waypoint() {
    let map = default_level_map();
    let terrain = TerrainMap::for_level(map.clone());
    // Positions within 36 units (Chebyshev) of player_spawn are guaranteed
    // clear because static roadblock definitions leave the spawn lane open.
    let start = map.player_spawn + Vec2::new(-30.0, 0.0);
    let goal = map.player_spawn + Vec2::new(30.0, 0.0);

    assert!(terrain.segment_is_walkable(start, goal));
    assert_eq!(terrain.next_waypoint(start, goal), goal);
  }

  #[test]
  fn movement_into_obstacle_is_blocked() {
    let map = default_level_map();
    let obs = map.obstacles[0]; // always Rectangle (index 0 in shape cycle)
    let terrain = TerrainMap::for_level(map);
    // Start just outside the obstacle's left edge
    let start = clamp_to_playable_area(Vec2::new(
      obs.center.x - obs.size.x * 0.5 - ACTOR_RADIUS - 2.0,
      obs.center.y,
    ));
    // Try to push through the obstacle horizontally
    let movement = Vec2::new(obs.size.x, 0.0);
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
  fn level_maps_change_walkable_layout() {
    let crossroads = TerrainMap::for_level(level_map(0));
    let m1 = level_map(1);
    let switchback = TerrainMap::for_level(m1.clone());
    let switchback_obstacle = m1.obstacles[0];

    assert!(crossroads.segment_is_walkable(Vec2::ZERO, Vec2::ZERO));
    assert!(
      !switchback.segment_is_walkable(switchback_obstacle.center, switchback_obstacle.center)
    );
    assert!(matches!(
      voxel_at_for_map(&m1, obstacle_center_voxel(switchback_obstacle)),
      WorldVoxel::Solid(v) if v == switchback_obstacle.material
    ));
  }

  #[test]
  fn blocked_segment_routes_to_next_walkable_cell() {
    let map = default_level_map();
    let obs = map.obstacles[0]; // Rectangle
    let terrain = TerrainMap::for_level(map);
    // Place start and goal on opposite sides of the obstacle
    let start = clamp_to_playable_area(Vec2::new(obs.center.x - obs.size.x, obs.center.y));
    let goal = clamp_to_playable_area(Vec2::new(obs.center.x + obs.size.x, obs.center.y));

    assert!(!terrain.segment_is_walkable(start, goal));

    let waypoint = terrain.next_waypoint(start, goal);

    assert_ne!(waypoint, start);
    assert_ne!(waypoint, goal);
    assert!(terrain.segment_is_walkable(start, waypoint));
  }

  #[test]
  fn voxel_lookup_builds_ground_and_obstacle_columns() {
    for i in 0 .. level_count() {
      let map = level_map(i);
      assert_eq!(
        voxel_at_for_map(&map, IVec3::new(0, -1, 0)),
        WorldVoxel::Solid(map.path_material)
      );
      if !map.obstacles.is_empty() {
        let obs = map.obstacles[0];
        let obstacle_voxel = obstacle_center_voxel(obs);
        assert_eq!(
          voxel_at_for_map(&map, obstacle_voxel),
          WorldVoxel::Solid(obs.material)
        );
        assert_eq!(
          voxel_at_for_map(&map, obstacle_voxel.with_y(map.obstacle_height)),
          WorldVoxel::Air
        );
      }
    }
  }

  #[test]
  fn level_maps_have_distinct_visual_terrain() {
    for i in 0 .. level_count() {
      for j in 0 .. level_count() {
        if i != j {
          let a = level_map(i);
          let b = level_map(j);
          assert_ne!(
            a.ground_material, b.ground_material,
            "levels {} and {} share ground",
            i, j
          );
        }
      }
    }

    for i in 0 .. level_count() {
      let map = level_map(i);
      let v = voxel_at_for_map(&map, IVec3::new(0, -1, 0));
      assert!(matches!(v, WorldVoxel::Solid(_)));
    }
  }

  #[test]
  fn each_level_uses_distinct_obstacle_shapes_positions_and_materials() {
    for i in 0 .. level_count() {
      let map = level_map(i);
      let first = map.obstacles[0];

      assert!(
        map
          .obstacles
          .iter()
          .any(|obstacle| obstacle.shape != first.shape),
        "level {} only uses {:?} obstacles",
        i,
        first.shape
      );
      assert!(
        map
          .obstacles
          .iter()
          .any(|obstacle| obstacle.material != first.material),
        "level {} only uses one obstacle material",
        i
      );

      for left in 0 .. map.obstacles.len() {
        for right in left + 1 .. map.obstacles.len() {
          assert_ne!(
            map.obstacles[left].center, map.obstacles[right].center,
            "level {} has duplicate obstacle center at indexes {} and {}",
            i, left, right
          );
        }
      }
    }
  }

  #[test]
  fn level_obstacle_counts_are_distinct() {
    let mut counts = vec![];

    for i in 0 .. level_count() {
      let map = level_map(i);
      assert!(
        !counts.contains(&map.obstacles.len()),
        "level {} repeats obstacle count {}",
        i,
        map.obstacles.len()
      );
      counts.push(map.obstacles.len());
    }
  }

  #[test]
  fn player_spawns_do_not_overlap_roadblocks() {
    for i in 0 .. level_count() {
      let map = level_map(i);
      assert!(
        !point_overlaps_any_obstacle(map.player_spawn, &map.obstacles, ACTOR_RADIUS),
        "level {} player spawn overlaps a roadblock",
        i
      );
    }
  }

  #[test]
  fn obstacle_centers_are_unique_across_all_levels() {
    let mut centers = vec![];

    for i in 0 .. level_count() {
      let map = level_map(i);
      for obstacle in &map.obstacles {
        assert!(
          !centers.contains(&obstacle.center),
          "level {} reuses obstacle center {:?}",
          i,
          obstacle.center
        );
        centers.push(obstacle.center);
      }
    }
  }

  #[test]
  fn every_level_has_a_different_roadblock_coordinate_set() {
    for left in 0 .. level_count() {
      for right in left + 1 .. level_count() {
        let left_map = level_map(left);
        let right_map = level_map(right);
        let left_centers: Vec<Vec2> = left_map
          .obstacles
          .iter()
          .map(|obstacle| obstacle.center)
          .collect();
        let right_centers: Vec<Vec2> = right_map
          .obstacles
          .iter()
          .map(|obstacle| obstacle.center)
          .collect();

        assert_ne!(
          left_centers, right_centers,
          "levels {} and {} use the same roadblock coordinate set",
          left, right
        );
      }
    }
  }

  #[test]
  fn level_maps_increase_difficulty_progressively() {
    for i in 1 .. level_count() {
      let prev = level_map(i - 1);
      let curr = level_map(i);
      assert!(
        curr.monsters.len() >= prev.monsters.len(),
        "level {} has fewer monsters than {}",
        i,
        i - 1
      );
    }
  }

  #[test]
  fn all_levels_have_unique_patterns() {
    let mut patterns = vec![];
    for i in 0 .. level_count() {
      let map = level_map(i);
      assert!(
        !patterns.contains(&map.pattern),
        "level {} has duplicate pattern {:?}",
        i,
        map.pattern
      );
      patterns.push(map.pattern);
    }
  }

  #[test]
  fn visual_ground_extends_beyond_arena_to_fill_view() {
    let near_outside_arena = IVec3::new(
      ((ARENA_HALF_SIZE.x + VISUAL_GROUND_PADDING * 0.5) / VOXEL_WORLD_SCALE) as i32,
      -1,
      0,
    );
    let far_outside_arena = IVec3::new(
      ((ARENA_HALF_SIZE.x + VISUAL_GROUND_PADDING * 2.0) / VOXEL_WORLD_SCALE) as i32,
      -1,
      0,
    );
    let map = default_level_map();

    assert!(matches!(
      voxel_at_for_map(&map, near_outside_arena),
      WorldVoxel::Solid(_)
    ));
    assert_eq!(
      voxel_at_for_map(&map, near_outside_arena + IVec3::Y),
      WorldVoxel::Air
    );
    assert_eq!(voxel_at_for_map(&map, far_outside_arena), WorldVoxel::Air);
  }
}
