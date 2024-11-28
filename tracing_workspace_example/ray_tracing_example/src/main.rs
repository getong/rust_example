mod camera;
mod color;
mod common;
mod hittable;
mod hittable_list;
mod material;
mod ray;
mod sphere;
mod vec3;

use std::io;
// use std::rc::Rc;
use std::sync::Arc;

use camera::Camera;
use color::Color;
use hittable::{HitRecord, Hittable};
use hittable_list::HittableList;
use material::{Dielectric, Lambertian, Metal};
use ray::Ray;
use rayon::prelude::*;
use sphere::Sphere;
use vec3::Point3;
// use vec3::Point3;

fn ray_color(r: &Ray, world: &dyn Hittable, depth: i32) -> Color {
  // If we've exceeded the ray bounce limit, no more light is gathered
  if depth <= 0 {
    return Color::new(0.0, 0.0, 0.0);
  }
  // fn ray_color(r: &Ray, world: &dyn Hittable) -> Color {
  // fn ray_color(r: &Ray) -> Color {
  // if hit_sphere(Point3::new(0.0, 0.0, -1.0), 0.5, r) {
  //     return Color::new(1.0, 0.0, 0.0);
  // }
  // let t = hit_sphere(Point3::new(0.0, 0.0, -1.0), 0.5, r);
  // if t > 0.0 {
  //     let n = vec3::unit_vector(r.at(t) - Vec3::new(0.0, 0.0, -1.0));
  //     return 0.5 * Color::new(n.x() + 1.0, n.y() + 1.0, n.z() + 1.0);
  // }
  let mut rec = HitRecord::new();
  if world.hit(r, 0.0, common::INFINITY, &mut rec) {
    // return 0.5 * (rec.normal + Color::new(1.0, 1.0, 1.0));
    // return 0.5 * ray_color(&Ray::new(rec.p, direction), world);
    // let direction = rec.normal + vec3::random_in_unit_sphere();
    // return 0.5 * ray_color(&Ray::new(rec.p, direction), world, depth - 1);

    let mut attenuation = Color::default();
    let mut scattered = Ray::default();
    if rec
      .mat
      .as_ref()
      .unwrap()
      .scatter(r, &rec, &mut attenuation, &mut scattered)
    {
      return attenuation * ray_color(&scattered, world, depth - 1);
    }
    return Color::new(0.0, 0.0, 0.0);
  }

  let unit_direction = vec3::unit_vector(r.direction());
  let t = 0.5 * (unit_direction.y() + 1.0);
  (1.0 - t) * Color::new(1.0, 1.0, 1.0) + t * Color::new(0.5, 0.7, 1.0)
}

fn random_scene() -> HittableList {
  let mut world = HittableList::new();

  // let ground_material = Rc::new(Lambertian::new(Color::new(0.5, 0.5, 0.5)));
  let ground_material = Arc::new(Lambertian::new(Color::new(0.5, 0.5, 0.5)));
  world.add(Box::new(Sphere::new(
    Point3::new(0.0, -1000.0, 0.0),
    1000.0,
    ground_material,
  )));

  for a in -11 .. 11 {
    for b in -11 .. 11 {
      let choose_mat = common::random_double();
      let center = Point3::new(
        a as f64 + 0.9 * common::random_double(),
        0.2,
        b as f64 + 0.9 * common::random_double(),
      );

      if (center - Point3::new(4.0, 0.2, 0.0)).length() > 0.9 {
        if choose_mat < 0.8 {
          // Diffuse
          let albedo = Color::random() * Color::random();
          // let sphere_material = Rc::new(Lambertian::new(albedo));
          let sphere_material = Arc::new(Lambertian::new(albedo));
          world.add(Box::new(Sphere::new(center, 0.2, sphere_material)));
        } else if choose_mat < 0.95 {
          // Metal
          let albedo = Color::random_range(0.5, 1.0);
          let fuzz = common::random_double_range(0.0, 0.5);
          let sphere_material = Arc::new(Metal::new(albedo, fuzz));
          world.add(Box::new(Sphere::new(center, 0.2, sphere_material)));
        } else {
          // Glass
          let sphere_material = Arc::new(Dielectric::new(1.5));
          world.add(Box::new(Sphere::new(center, 0.2, sphere_material)));
        }
      }
    }
  }

  let material1 = Arc::new(Dielectric::new(1.5));
  world.add(Box::new(Sphere::new(
    Point3::new(0.0, 1.0, 0.0),
    1.0,
    material1,
  )));

  let material2 = Arc::new(Lambertian::new(Color::new(0.4, 0.2, 0.1)));
  world.add(Box::new(Sphere::new(
    Point3::new(-4.0, 1.0, 0.0),
    1.0,
    material2,
  )));

  let material3 = Arc::new(Metal::new(Color::new(0.7, 0.6, 0.5), 0.0));
  world.add(Box::new(Sphere::new(
    Point3::new(4.0, 1.0, 0.0),
    1.0,
    material3,
  )));

  world
}

// fn hit_sphere(center: Point3, radius: f64, r: &Ray) -> f64 {
//     let oc = r.origin() - center;
//     let a = r.direction().length_squared();
//     let half_b = vec3::dot(oc, r.direction());
//     let c = oc.length_squared() - radius * radius;
//     let discriminant = half_b * half_b - a * c;
//     if discriminant < 0.0 {
//         -1.0
//     } else {
//         (-half_b - f64::sqrt(discriminant)) / a
//     }
// }

fn main() {
  // Image

  const ASPECT_RATIO: f64 = 3.0 / 2.0;
  const IMAGE_WIDTH: i32 = 1200;
  const IMAGE_HEIGHT: i32 = (IMAGE_WIDTH as f64 / ASPECT_RATIO) as i32;
  const SAMPLES_PER_PIXEL: i32 = 500;
  const MAX_DEPTH: i32 = 50;
  // World

  // let r = f64::cos(common::PI / 4.0);
  // let mut world = HittableList::new();
  // let material_ground = Rc::new(Lambertian::new(Color::new(0.8, 0.8, 0.0)));
  // let material_center = Rc::new(Lambertian::new(Color::new(0.1, 0.2, 0.5)));
  // let material_left = Rc::new(Dielectric::new(1.5));
  // let material_right = Rc::new(Metal::new(Color::new(0.8, 0.6, 0.2), 0.0));

  // world.add(Box::new(Sphere::new(
  //     Point3::new(0.0, -100.5, -1.0),
  //     100.0,
  //     material_ground,
  // )));
  // world.add(Box::new(Sphere::new(
  //     Point3::new(0.0, 0.0, -1.0),
  //     0.5,
  //     material_center,
  // )));
  // world.add(Box::new(Sphere::new(
  //     Point3::new(-1.0, 0.0, -1.0),
  //     0.5,
  //     material_left.clone(),
  // )));
  // world.add(Box::new(Sphere::new(
  //     Point3::new(-1.0, 0.0, -1.0),
  //     -0.45,
  //     material_left,
  // )));
  // world.add(Box::new(Sphere::new(
  //     Point3::new(1.0, 0.0, -1.0),
  //     0.5,
  //     material_right,
  // )));
  let world = random_scene();

  // world.add(Box::new(Sphere::new(Point3::new(0.0, 0.0, -1.0), 0.5)));
  // world.add(Box::new(Sphere::new(Point3::new(0.0, -100.5, -1.0), 100.0)));

  // Camera

  let lookfrom = Point3::new(13.0, 2.0, 3.0);
  let lookat = Point3::new(0.0, 0.0, 0.0);
  let vup = Point3::new(0.0, 1.0, 0.0);
  let dist_to_focus = 10.0;
  let aperture = 0.1;

  let cam = Camera::new(
    lookfrom,
    lookat,
    vup,
    20.0,
    ASPECT_RATIO,
    aperture,
    dist_to_focus,
  );
  // let viewport_height = 2.0;
  // let viewport_width = ASPECT_RATIO * viewport_height;
  // let focal_length = 1.0;

  // let origin = Point3::new(0.0, 0.0, 0.0);
  // let horizontal = Vec3::new(viewport_width, 0.0, 0.0);
  // let vertical = Vec3::new(0.0, viewport_height, 0.0);
  // let lower_left_corner =
  //     origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new(0.0, 0.0, focal_length);

  // Render

  print!("P3\n{} {}\n255\n", IMAGE_WIDTH, IMAGE_HEIGHT);
  // let b = 0.25;
  for j in (0 .. IMAGE_HEIGHT).rev() {
    eprint!("\rScanlines remaining: {} ", j);
    // for i in 0..IMAGE_WIDTH {
    //     // let u = i as f64 / (IMAGE_WIDTH - 1) as f64;
    //     // let v = j as f64 / (IMAGE_HEIGHT - 1) as f64;
    //     // let r = Ray::new(
    //     //     origin,
    //     //     lower_left_corner + u * horizontal + v * vertical - origin,
    //     // );
    //     // // let pixel_color = ray_color(&r);
    //     // let pixel_color = ray_color(&r, &world);
    //     // color::write_color(&mut io::stdout(), pixel_color);

    //     // let ir = (255.999 * r) as i32;
    //     // let ig = (255.999 * g) as i32;
    //     // let ib = (255.999 * b) as i32;

    //     // print!("{} {} {}\n", ir, ig, ib);
    //     let mut pixel_color = Color::new(0.0, 0.0, 0.0);
    //     for _ in 0..SAMPLES_PER_PIXEL {
    //         let u = (i as f64 + common::random_double()) / (IMAGE_WIDTH - 1) as f64;
    //         let v = (j as f64 + common::random_double()) / (IMAGE_HEIGHT - 1) as f64;
    //         let r = cam.get_ray(u, v);
    //         pixel_color += ray_color(&r, &world, MAX_DEPTH);
    //         // pixel_color += ray_color(&r, &world);
    //     }
    //     color::write_color(&mut io::stdout(), pixel_color, SAMPLES_PER_PIXEL);
    // }
    let pixel_colors: Vec<_> = (0 .. IMAGE_WIDTH)
      .into_par_iter()
      .map(|i| {
        let mut pixel_color = Color::new(0.0, 0.0, 0.0);
        for _ in 0 .. SAMPLES_PER_PIXEL {
          let u = ((i as f64) + common::random_double()) / (IMAGE_WIDTH - 1) as f64;
          let v = ((j as f64) + common::random_double()) / (IMAGE_HEIGHT - 1) as f64;
          let r = cam.get_ray(u, v);
          pixel_color += ray_color(&r, &world, MAX_DEPTH);
        }
        pixel_color
      })
      .collect();
    for pixel_color in pixel_colors {
      color::write_color(&mut io::stdout(), pixel_color, SAMPLES_PER_PIXEL);
    }
  }
}

// cargo run > image.ppm
