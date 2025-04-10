use many_cpus::ProcessorSet;

fn main() {
  let all_processors = ProcessorSet::all();
  let num_workers = all_processors.len();
  println!("Starting {} worker threads", num_workers);
}
