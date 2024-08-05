// use std::sync::{Arc, Mutex};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

fn main() {
  // println!("Hello, world!");
  let num_cpus = num_cpus::get();
  rayon::ThreadPoolBuilder::new()
    .num_threads(num_cpus)
    .build_global()
    .unwrap();

  // let a = Arc::new(Mutex::new(0));
  perform_parallel_task();
}

fn perform_parallel_task() {
  let data: Vec<u64> = (9995 .. 10000).collect();

  let result: Vec<u64> = data.par_iter().map(|&x| x * x).collect();

  // Optionally, do something with the result
  println!("{:?}", result);
}
