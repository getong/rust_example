use std::time::Instant;

use rayon::prelude::*;

fn main() {
  // Create a large vector of numbers
  let large_vec: Vec<u64> = (1 .. 1000000).collect();

  // Start timing
  let start = Instant::now();

  // Perform a CPU-intensive task in parallel
  let _squared_vec: Vec<u64> = large_vec
    .par_iter() // Use a parallel iterator
    .map(|&num| num * num) // Square each number
    .collect(); // Collect the results into a new vector

  // Stop timing
  let duration = start.elapsed();

  // Print the time taken
  println!("Time taken: {:?}", duration);

  // Optionally, print part of the resulting vector
  // println!("First 10 squares: {:?}", &squared_vec[..10]);
}
