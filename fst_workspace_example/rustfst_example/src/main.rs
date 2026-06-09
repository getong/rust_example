use anyhow::Result;
use rustfst::{
  algorithms::{determinize::determinize, rm_epsilon::rm_epsilon},
  prelude::*,
};

fn main() -> Result<()> {
  // Creates a empty wFST
  let mut fst = VectorFst::<TropicalWeight>::new();

  // Add some states
  let s0 = fst.add_state();
  let s1 = fst.add_state();
  let s2 = fst.add_state();

  // Set s0 as the start state
  fst.set_start(s0)?;

  // Add a transition from s0 to s1
  fst.add_tr(s0, Tr::new(3, 5, 10.0, s1))?;

  // Add a transition from s0 to s2
  fst.add_tr(s0, Tr::new(5, 7, 18.0, s2))?;

  // Set s1 and s2 as final states
  fst.set_final(s1, 31.0)?;
  fst.set_final(s2, 45.0)?;

  // Iter over all the paths in the wFST
  for p in fst.paths_iter() {
    println!("{:?}", p);
  }

  // A lot of operations are available to modify/optimize the FST.
  // Here are a few examples :

  // - Remove useless states.
  connect(&mut fst)?;

  // - Optimize the FST by merging states with the same behaviour.
  minimize(&mut fst)?;

  // - Copy all the input labels in the output.
  project(&mut fst, ProjectType::ProjectInput);

  // - Remove epsilon transitions.
  rm_epsilon(&mut fst)?;

  // - Compute an equivalent FST but deterministic.
  fst = determinize(&fst)?;

  // Iter over all the paths in the wFST
  for p in fst.paths_iter() {
    println!("{:?}", p);
  }

  Ok(())
}
