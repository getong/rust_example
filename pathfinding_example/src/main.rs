fn main() {
  match pathfinding_example::run_pathfinding_demo(pathfinding_example::PathfindingAlgorithm::AStar)
  {
    Some(result) => {
      println!("{} demo", result.algorithm_name);
      println!("start: {:?}", result.start);
      println!("goal: {:?}", result.goal);
      println!("path: {:?}", result.path);
      println!("total cost: {}", result.total_cost);
    }
    None => {
      println!("No path found for the selected demo.");
    }
  }
}
