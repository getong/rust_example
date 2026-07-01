use many_cpus::SystemHardware;

fn main() {
  let all_processors = SystemHardware::current().processors();
  let num_workers = all_processors.len();
  println!("Starting {} worker threads", num_workers);
}
