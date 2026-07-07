use std::f32::consts::PI;

use bevy::prelude::*;

const SUN_PASS_SECONDS: f32 = 10.0;
const SUN_SWEEP_HALF_WIDTH: f32 = 22.0;
const SUN_HORIZON_HEIGHT: f32 = 4.0;
const SUN_NOON_HEIGHT: f32 = 28.0;
const SUN_Z: f32 = 8.0;
const SUN_TARGET: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const SUNRISE_ILLUMINANCE: f32 = 8_000.0;
const NOON_ILLUMINANCE: f32 = 24_000.0;

#[derive(Component)]
pub(crate) struct SunPath;

pub(crate) fn initial_sun_transform() -> Transform {
  sun_transform_at(0.0)
}

pub(crate) fn animate_sunlight(
  time: Res<Time>,
  mut suns: Query<(&mut Transform, &mut DirectionalLight), With<SunPath>>,
) {
  let elapsed_secs = time.elapsed_secs_wrapped();
  for (mut transform, mut light) in &mut suns {
    let phase = sun_phase(elapsed_secs);
    *transform = sun_transform_from_phase(phase);
    light.illuminance = sun_illuminance(phase);
    light.color = sun_color(phase);
  }
}

fn sun_transform_at(elapsed_secs: f32) -> Transform {
  sun_transform_from_phase(sun_phase(elapsed_secs))
}

fn sun_transform_from_phase(phase: f32) -> Transform {
  Transform::from_translation(sun_position(phase)).looking_at(SUN_TARGET, Vec3::Y)
}

fn sun_position(phase: f32) -> Vec3 {
  let x = lerp(
    -SUN_SWEEP_HALF_WIDTH,
    SUN_SWEEP_HALF_WIDTH,
    smoothstep(phase),
  );
  let height_ratio = (phase * PI).sin().max(0.0);
  let y = lerp(SUN_HORIZON_HEIGHT, SUN_NOON_HEIGHT, height_ratio);

  Vec3::new(x, y, SUN_Z)
}

fn sun_illuminance(phase: f32) -> f32 {
  let height_ratio = (phase * PI).sin().max(0.0);
  lerp(SUNRISE_ILLUMINANCE, NOON_ILLUMINANCE, height_ratio)
}

fn sun_color(phase: f32) -> Color {
  let height_ratio = (phase * PI).sin().max(0.0);
  Color::srgb(
    1.0,
    lerp(0.58, 0.96, height_ratio),
    lerp(0.34, 0.86, height_ratio),
  )
}

fn sun_phase(elapsed_secs: f32) -> f32 {
  (elapsed_secs / SUN_PASS_SECONDS).fract()
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
  start + (end - start) * t
}

fn smoothstep(t: f32) -> f32 {
  t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sun_rises_from_left_to_noon() {
    let sunrise = sun_position(0.0);
    let noon = sun_position(0.5);

    assert_eq!(sunrise.x, -SUN_SWEEP_HALF_WIDTH);
    assert_eq!(sunrise.y, SUN_HORIZON_HEIGHT);
    assert_eq!(noon.x, 0.0);
    assert_eq!(noon.y, SUN_NOON_HEIGHT);
  }

  #[test]
  fn sun_sets_on_the_right() {
    let sunset = sun_position(0.999);

    assert!(sunset.x > SUN_SWEEP_HALF_WIDTH - 0.1);
    assert!(sunset.y < SUN_HORIZON_HEIGHT + 0.1);
  }
}
