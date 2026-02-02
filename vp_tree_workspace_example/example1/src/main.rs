use vp_tree::*;

struct Point {
  x: f64,
  y: f64,
  z: f64,
}

impl Distance<Point> for Point {
  fn distance(&self, other: &Point) -> f64 {
    self.distance_heuristic(other).sqrt()
  }

  fn distance_heuristic(&self, other: &Point) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    let dz = self.z - other.z;
    dx * dx + dy * dy + dz * dz
  }
}

fn main() {
  let random_points = (0 .. 10_000)
    .map(|_| Point {
      x: fastrand::f64() * 1000.0,
      y: fastrand::f64() * 1000.0,
      z: fastrand::f64() * 1000.0,
    })
    .collect::<Vec<_>>();

  let target_point = Point {
    x: 500.0,
    y: 500.0,
    z: 500.0,
  };

  // Build VpTree using 4 threads
  let vp_tree = VpTree::new_parallel(random_points, 4);

  let _nearest_neighbor = vp_tree.nearest_neighbor(&target_point);
  let _k_closest_neighbors = vp_tree.querry(&target_point, Querry::k_nearest_neighbors(5));
  let _in_radius = vp_tree.querry(&target_point, Querry::neighbors_within_radius(100.0));
}
