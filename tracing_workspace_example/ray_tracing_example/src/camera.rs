use crate::{
  common,
  ray::Ray,
  vec3::{self, Point3, Vec3},
};

pub struct Camera {
  origin: Point3,
  lower_left_corner: Point3,
  horizontal: Vec3,
  vertical: Vec3,
  u: Vec3,
  v: Vec3,
  lens_radius: f64,
}

impl Camera {
  pub fn new(
    lookfrom: Point3,
    lookat: Point3,
    vup: Vec3,
    vfov: f64, // Vertical field-of-view in degrees
    aspect_ratio: f64,
    aperture: f64,
    focus_dist: f64,
  ) -> Camera {
    let theta = common::degrees_to_radians(vfov);
    let h = f64::tan(theta / 2.0);
    let viewport_height = 2.0 * h;
    let viewport_width = aspect_ratio * viewport_height;
    let w = vec3::unit_vector(lookfrom - lookat);
    let u = vec3::unit_vector(vec3::cross(vup, w));
    let v = vec3::cross(w, u);

    let origin = lookfrom;
    let horizontal = focus_dist * viewport_width * u;
    let vertical = focus_dist * viewport_height * v;
    let lower_left_corner = origin - horizontal / 2.0 - vertical / 2.0 - focus_dist * w;

    let lens_radius = aperture / 2.0;

    Camera {
      origin,
      lower_left_corner,
      horizontal,
      vertical,
      u,
      v,
      lens_radius,
    }
  }

  pub fn get_ray(&self, s: f64, t: f64) -> Ray {
    let rd = self.lens_radius * vec3::random_in_unit_disk();
    let offset = self.u * rd.x() + self.v * rd.y();
    Ray::new(
      self.origin + offset,
      self.lower_left_corner + s * self.horizontal + t * self.vertical - self.origin - offset,
    )
  }
}
