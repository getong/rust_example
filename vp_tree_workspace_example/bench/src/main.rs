use std::collections::BinaryHeap;

use vp_tree::*;

#[derive(Debug, Clone)]
struct Point {
  x: f64,
  y: f64,
}

impl Distance<Point> for Point {
  fn distance(&self, other: &Point) -> f64 {
    self.distance_heuristic(other).sqrt()
  }

  fn distance_heuristic(&self, other: &Point) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    dx * dx + dy * dy
  }
}

fn main() {
  let num_points = 1_000_000;

  let random_points = (0 .. num_points)
    .map(|_| Point {
      x: fastrand::f64() * 1000.0,
      y: fastrand::f64() * 1000.0,
    })
    .collect::<Vec<_>>();

  let target_point = Point { x: 500.0, y: 500.0 };

  println!("Baseline linear search:");

  let start = std::time::Instant::now();
  let nearest_linear = find_nearest_neighbor_linear(&random_points, &target_point);
  let baseline_duration = start.elapsed();
  println!(
    "Time taken for linear search with {} points: {:?}, Result: {:?}",
    num_points, baseline_duration, nearest_linear
  );

  let start = std::time::Instant::now();
  let k_closest_linear = find_k_closest_linear(&random_points, &target_point, 5);
  let k_baseline_duration = start.elapsed();
  println!(
    "Time taken to find 5 closest neighbors linearly: {:?}. Result count: {}",
    k_baseline_duration,
    k_closest_linear.len()
  );

  let start = std::time::Instant::now();
  let in_radius_linear = find_in_radius_linear(&random_points, &target_point, 2.0);
  let radius_baseline_duration = start.elapsed();
  println!(
    "Time taken to find points within radius 2.0 linearly: {:?}, found {} points",
    radius_baseline_duration,
    in_radius_linear.len()
  );

  println!("\nVpTree search:");

  let random_points_clone = random_points.clone();

  let start = std::time::Instant::now();
  let vp_tree = vp_tree::VpTree::new_parallel(random_points, 16);
  let duration = start.elapsed();
  println!(
    "Time taken to build VpTree with {} points on 16 threads: {:?}",
    num_points, duration
  );

  let start = std::time::Instant::now();
  let _vp_tree = vp_tree::VpTree::new(random_points_clone);
  let duration = start.elapsed();
  println!(
    "Time taken to build VpTree with {} points on single thread: {:?}",
    num_points, duration
  );

  let start = std::time::Instant::now();
  let nearest_neighbor = vp_tree.nearest_neighbor(&target_point);
  let duration = start.elapsed();
  println!(
    "Time taken to search nearest neighbor: {:?}, {:.2?} times faster than linear search. Result: \
     {:?}",
    duration,
    baseline_duration.as_secs_f64() / duration.as_secs_f64(),
    nearest_neighbor
  );

  let start = std::time::Instant::now();
  let k_closest_neighbors = vp_tree.querry(&target_point, Querry::k_nearest_neighbors(5));
  let duration = start.elapsed();
  println!(
    "Time taken to search 5 closest neighbors: {:?}, {:.2?} times faster than linear search. \
     Result count: {}",
    duration,
    k_baseline_duration.as_secs_f64() / duration.as_secs_f64(),
    k_closest_neighbors.len()
  );

  let start = std::time::Instant::now();
  let in_radius = vp_tree.querry(&target_point, Querry::neighbors_within_radius(2.0));
  let duration = start.elapsed();
  println!(
    "Time taken to search points within radius 2.0: {:?}, {:.2?} times faster than linear search. \
     Result count: {}",
    duration,
    radius_baseline_duration.as_secs_f64() / duration.as_secs_f64(),
    in_radius.len()
  );
}

fn find_nearest_neighbor_linear<'a>(points: &'a Vec<Point>, target: &Point) -> Option<&'a Point> {
  points.iter().min_by(|a, b| {
    let dist_a = a.distance_heuristic(&target);
    let dist_b = b.distance_heuristic(&target);
    dist_a.partial_cmp(&dist_b).unwrap()
  })
}

fn find_k_closest_linear<'a>(points: &'a Vec<Point>, target: &Point, k: usize) -> Vec<&'a Point> {
  let mut binary_heap = BinaryHeap::new();

  for point in points.iter() {
    let distance = point.distance_heuristic(target);
    binary_heap.push(HeapItemHelper { distance, point });
    if binary_heap.len() > k {
      binary_heap.pop();
    }
  }

  binary_heap
    .into_sorted_vec()
    .into_iter()
    .take(k)
    .map(|item| item.point)
    .collect()
}

struct HeapItemHelper<'a> {
  distance: f64,
  point: &'a Point,
}

impl<'a> PartialEq for HeapItemHelper<'a> {
  fn eq(&self, other: &Self) -> bool {
    self.distance == other.distance
  }
}

impl<'a> Eq for HeapItemHelper<'a> {}

impl<'a> PartialOrd for HeapItemHelper<'a> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    // Reverse order for min-heap behavior
    other.distance.partial_cmp(&self.distance)
  }
}

impl<'a> Ord for HeapItemHelper<'a> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    // Reverse order for min-heap behavior
    other.distance.partial_cmp(&self.distance).unwrap()
  }
}

fn find_in_radius_linear<'a>(
  points: &'a Vec<Point>,
  target: &Point,
  radius: f64,
) -> Vec<&'a Point> {
  points
    .iter()
    .filter(|p| p.distance_heuristic(&target) <= radius * radius)
    .collect()
}
