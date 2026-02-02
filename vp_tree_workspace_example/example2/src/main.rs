use vp_tree::*;

struct DataPoint {
  x: f64,
  y: f64,
  _data: String,
}

struct Point {
  x: f64,
  y: f64,
}

impl Distance<DataPoint> for DataPoint {
  fn distance_heuristic(&self, other: &DataPoint) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    dx * dx + dy * dy
  }

  fn distance(&self, other: &DataPoint) -> f64 {
    self.distance_heuristic(other).sqrt()
  }
}

impl Distance<DataPoint> for Point {
  fn distance(&self, other: &DataPoint) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    ((dx * dx) + (dy * dy)).sqrt()
  }
}

fn main() {
  let vp_tree = (0 .. 10_000)
    .map(|i| DataPoint {
      x: fastrand::f64() * 1000.0,
      y: fastrand::f64() * 1000.0,
      _data: format!("Point {}", i),
    })
    .collect::<VpTree<DataPoint>>();

  let target_point = Point { x: 500.0, y: 500.0 };

  let _nearest_neighbor = vp_tree.nearest_neighbor(&target_point);
  let _k_closest_neighbors = vp_tree.querry(&target_point, Querry::k_nearest_neighbors(5));
  let _in_radius = vp_tree.querry(&target_point, Querry::neighbors_within_radius(100.0));

  let full_querry = Querry::k_nearest_neighbors(5)
    .within_radius(100.0)
    .sorted()
    .exclusive();
  let _custom_querry = vp_tree.querry(&target_point, full_querry);
}
