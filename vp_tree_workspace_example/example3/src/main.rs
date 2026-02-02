use vp_tree::Distance;

struct Point {
    coordinates:  [f64; 10],
}

impl Distance<Point> for Point {
    fn distance(&self, other: &Point) -> f64 {
        self.distance_heuristic(other).sqrt()
    }

    fn distance_heuristic(&self, other: &Point) -> f64 {
        self.coordinates.iter().zip(other.coordinates.iter())
            .map(|(a, b)| {
                let diff = a - b;
                diff * diff
            })
            .sum()
    }
}

fn main() {
    let random_points = (0..10_000)
        .map(|_| Point {
            coordinates: [(); 10].map(|_| fastrand::f64() * 1000.0),
        })
        .collect::<Vec<_>>();

    let target_point = Point { coordinates: [500.0; 10] };
    
    // Build VpTree using 4 threads
    let vp_tree = vp_tree::VpTree::new_index_parallel(&random_points, 4);
    
    let _nearest_neighbor = vp_tree.nearest_neighbor(&target_point);
    let _k_closest_neighbors = vp_tree.querry(&target_point, vp_tree::Querry::k_nearest_neighbors(5));
    let _in_radius = vp_tree.querry(&target_point, vp_tree::Querry::neighbors_within_radius(100.0));
}