use std::{
  collections::HashSet,
  hash::{Hash, Hasher},
};

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct Field([[bool; 7]; 7]);

impl Field {
  pub fn new() -> Field {
    Field([[false; 7]; 7])
  }

  fn mirror(&self) -> Field {
    let mut d = self.0;
    for i in 0 .. 7 {
      d[i].reverse();
    }
    Field(d)
  }

  fn rotate(&self) -> Field {
    let mut res = Field::new();
    for i in 0 .. 7 {
      for j in 0 .. 7 {
        res.0[j][6 - i] = self.0[i][j];
      }
    }
    res
  }
}

impl Hash for Field {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let mut transf = vec![*self];
    for _ in 0 .. 4 {
      transf.push(transf.last().unwrap().rotate());
    }
    for i in 0 .. 5 {
      transf.push(transf[i].mirror());
    }
    transf.iter().max().unwrap().0.hash(state);
  }
}

type Vertex = (usize, usize);
#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
pub struct Move {
  pub from: Vertex,
  pub to: Vertex,
}

pub fn find_solution_from_state(occ: &Field) -> Option<Vec<Move>> {
  rec_solve(&init_allowed_positions(), occ, &mut HashSet::new())
}

pub fn solve_solitaire() -> Vec<Move> {
  rec_solve(
    &init_allowed_positions(),
    &init_occupations(),
    &mut HashSet::new(),
  )
  .unwrap()
}

fn init_allowed_positions() -> Field {
  let mut res = Field::new();
  for i in 0 .. 7 {
    for j in 0 .. 7 {
      if i >= 2 && i <= 4 || j >= 2 && j <= 4 {
        res.0[i][j] = true;
      }
    }
  }
  res
}

fn init_occupations() -> Field {
  let mut res = init_allowed_positions();
  res.0[3][3] = false;
  res
}

fn rec_solve(allowed_pos: &Field, occ: &Field, pruning: &mut HashSet<Field>) -> Option<Vec<Move>> {
  if is_solved(occ) {
    Some(vec![])
  } else {
    for mv in find_allowed_moves(&allowed_pos, &occ).iter() {
      let mut occ_new = *occ;
      do_move(mv, &mut occ_new);
      if !pruning.contains(&occ_new) {
        if let Some(mut moves) = rec_solve(allowed_pos, &occ_new, pruning) {
          moves.insert(0, *mv);
          return Some(moves);
        } else {
          pruning.insert(occ_new);
        }
      }
    }
    None
  }
}

fn find_allowed_moves(allowed_pos: &Field, occ: &Field) -> HashSet<Move> {
  let mut res = HashSet::new();
  for x in 0 .. 7 {
    for y in 0 .. 7 {
      if allowed_pos.0[x][y] && !occ.0[x][y] {
        for i in [-1, 1] {
          let mut possible_from_over_values = vec![];
          let x_from = x as i32 + 2 * i;
          if x_from >= 0 && x_from < 7 {
            possible_from_over_values.push(((x_from as usize, y), ((x as i32 + i) as usize, y)))
          }
          let y_from = y as i32 + 2 * i;
          if y_from >= 0 && y_from < 7 {
            possible_from_over_values.push(((x, y_from as usize), (x, (y as i32 + i) as usize)))
          }
          for (from, over) in possible_from_over_values {
            if occ.0[from.0][from.1] && occ.0[over.0][over.1] {
              res.insert(Move { from, to: (x, y) });
            }
          }
        }
      }
    }
  }
  res
}

fn is_solved(occ: &Field) -> bool {
  let mut solution = Field::new();
  solution.0[3][3] = true;
  occ == &solution
}

fn do_move(mv: &Move, occ: &mut Field) {
  let over = if mv.from.0 == mv.to.0 {
    (
      mv.from.0,
      (mv.from.1 as i32 + i32::signum(mv.to.1 as i32 - mv.from.1 as i32)) as usize,
    )
  } else {
    (
      (mv.from.0 as i32 + i32::signum(mv.to.0 as i32 - mv.from.0 as i32)) as usize,
      mv.from.1,
    )
  };
  occ.0[mv.from.0][mv.from.1] = false;
  occ.0[over.0][over.1] = false;
  occ.0[mv.to.0][mv.to.1] = true;
}

fn main() {
  println!("{:?}", solve_solitaire());
}
