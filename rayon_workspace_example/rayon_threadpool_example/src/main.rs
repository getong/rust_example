fn main() {
  // println!("Hello, world!");
  let num_cpus = num_cpus::get();
  rayon::ThreadPoolBuilder::new()
    .num_threads(num_cpus)
    .build_global()
    .unwrap();
}
