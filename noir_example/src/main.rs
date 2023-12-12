use noir::prelude::*;

fn main() {
  // Convenience method to parse deployment config from CLI arguments
  let (config, args) = EnvironmentConfig::from_args();
  let mut env = StreamEnvironment::new(config);
  env.spawn_remote_workers();

  let result = env
    // Open and read file line by line in parallel
    .stream_file(&args[0])
    // Split into words
    .flat_map(|line| tokenize(&line))
    // Partition
    .group_by(|word| word.clone())
    // Count occurrences
    .fold(0, |count, _word| *count += 1)
    // Collect result
    .collect_vec();

  env.execute_blocking(); // Start execution (blocking)
  if let Some(result) = result.get() {
    // Print word counts
    result
      .into_iter()
      .for_each(|(word, count)| println!("{word}: {count}"));
  }
}

fn tokenize(s: &str) -> Vec<String> {
  // Simple tokenisation strategy
  s.split_whitespace().map(str::to_lowercase).collect()
}

// Execute on 6 local hosts `cargo run -- -l 6 input.txt`
