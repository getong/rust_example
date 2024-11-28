use std::io::Write;

use crate::{common, vec3::Vec3};

// Type alias
pub type Color = Vec3;

pub fn write_color(out: &mut impl Write, pixel_color: Color, samples_per_pixel: i32) {
  let mut r = pixel_color.x();
  let mut g = pixel_color.y();
  let mut b = pixel_color.z();

  // Divide the color by the number of samples
  let scale = 1.0 / samples_per_pixel as f64;
  r *= scale;
  g *= scale;
  b *= scale;

  // Write the translated [0, 255] value of each color component
  writeln!(
    out,
    "{} {} {}",
    (256.0 * common::clamp(r, 0.0, 0.999)) as i32,
    (256.0 * common::clamp(g, 0.0, 0.999)) as i32,
    (256.0 * common::clamp(b, 0.0, 0.999)) as i32,
  )
  .expect("writing color");
}
