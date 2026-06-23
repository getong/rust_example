//! Small demos for algorithms provided by the `pathfinding` crate.

use pathfinding::prelude::astar;

/// Pathfinding algorithms that can be demonstrated by this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathfindingAlgorithm {
  /// A* search with a Manhattan-distance heuristic on a grid map.
  AStar,
}

/// Result data produced by a pathfinding algorithm demo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoResult {
  /// Algorithm used to compute the path.
  pub algorithm: PathfindingAlgorithm,
  /// Human-readable name of the algorithm.
  pub algorithm_name: &'static str,
  /// First grid coordinate in the route.
  pub start: Position,
  /// Target grid coordinate.
  pub goal: Position,
  /// Path returned by the algorithm, including `start` and `goal`.
  pub path: Vec<Position>,
  /// Total movement cost for the returned path.
  pub total_cost: u32,
}

/// A coordinate in the demo grid.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Position {
  /// Horizontal grid coordinate.
  pub x: i32,
  /// Vertical grid coordinate.
  pub y: i32,
}

impl Position {
  fn manhattan_distance(self, other: Self) -> u32 {
    self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
  }
}

struct GridMap {
  width: i32,
  height: i32,
  blocked: &'static [Position],
}

impl GridMap {
  fn successors(&self, position: &Position) -> Vec<(Position, u32)> {
    const DIRECTIONS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    DIRECTIONS
      .into_iter()
      .map(|(dx, dy)| Position {
        x: position.x + dx,
        y: position.y + dy,
      })
      .filter(|next| self.contains(*next) && !self.is_blocked(*next))
      .map(|next| (next, 1))
      .collect()
  }

  fn contains(&self, position: Position) -> bool {
    (0 .. self.width).contains(&position.x) && (0 .. self.height).contains(&position.y)
  }

  fn is_blocked(&self, position: Position) -> bool {
    self.blocked.contains(&position)
  }
}

/// Runs one pathfinding algorithm demo.
///
/// This function is the extension point for future demos. Add a variant to
/// [`PathfindingAlgorithm`], implement a dedicated demo function, then dispatch
/// to it from this match.
pub fn run_pathfinding_demo(algorithm: PathfindingAlgorithm) -> Option<DemoResult> {
  match algorithm {
    PathfindingAlgorithm::AStar => astar_demo(),
  }
}

fn astar_demo() -> Option<DemoResult> {
  let grid = GridMap {
    width: 6,
    height: 5,
    blocked: &[
      Position { x: 1, y: 1 },
      Position { x: 2, y: 1 },
      Position { x: 3, y: 1 },
      Position { x: 3, y: 2 },
      Position { x: 3, y: 3 },
    ],
  };
  let start = Position { x: 0, y: 0 };
  let goal = Position { x: 5, y: 4 };

  astar(
    &start,
    |position| grid.successors(position),
    |position| position.manhattan_distance(goal),
    |position| *position == goal,
  )
  .map(|(path, total_cost)| DemoResult {
    algorithm: PathfindingAlgorithm::AStar,
    algorithm_name: "A*",
    start,
    goal,
    path,
    total_cost,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn astar_demo_finds_shortest_path_around_walls() {
    let blocked = [
      Position { x: 1, y: 1 },
      Position { x: 2, y: 1 },
      Position { x: 3, y: 1 },
      Position { x: 3, y: 2 },
      Position { x: 3, y: 3 },
    ];
    let Some(result) = run_pathfinding_demo(PathfindingAlgorithm::AStar) else {
      panic!("A* demo should find a path");
    };

    assert_eq!(result.algorithm, PathfindingAlgorithm::AStar);
    assert_eq!(result.algorithm_name, "A*");
    assert_eq!(result.start, Position { x: 0, y: 0 });
    assert_eq!(result.goal, Position { x: 5, y: 4 });
    assert_eq!(result.path.first(), Some(&result.start));
    assert_eq!(result.path.last(), Some(&result.goal));
    assert_eq!(result.total_cost, 9);
    assert_eq!(result.path.len() as u32, result.total_cost + 1);

    for pair in result.path.windows(2) {
      let [from, to] = pair else {
        panic!("windows(2) must return two positions");
      };

      assert_eq!(from.manhattan_distance(*to), 1);
      assert!(!blocked.contains(to));
    }
  }
}
