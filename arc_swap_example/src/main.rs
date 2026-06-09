use std::{
  sync::{Arc, RwLock, mpsc},
  thread,
  time::{Duration, Instant},
};

use arc_swap::ArcSwap;

#[derive(Debug)]
struct AppConfig {
  version: u64,
  feature_enabled: bool,
  endpoint: String,
}

fn main() {
  run_arc_swap_example();
  println!();
  run_rwlock_example();
}

fn run_arc_swap_example() {
  println!("=== ArcSwap: replace the whole config without blocking readers ===");

  let config = Arc::new(ArcSwap::from_pointee(make_config(1)));
  let (reader_ready_tx, reader_ready_rx) = mpsc::channel();

  let reader = {
    let config = Arc::clone(&config);

    thread::spawn(move || {
      let snapshot = config.load_full();
      println!("arc_swap reader got {}", config_summary(&snapshot));

      reader_ready_tx
        .send(())
        .expect("arc_swap reader failed to notify writer");

      thread::sleep(Duration::from_millis(200));

      println!(
        "arc_swap reader still uses old snapshot after sleep: {}",
        config_summary(&snapshot)
      );
    })
  };

  let writer = {
    let config = Arc::clone(&config);

    thread::spawn(move || {
      reader_ready_rx
        .recv()
        .expect("arc_swap writer failed to wait for reader");

      let started_at = Instant::now();
      config.store(Arc::new(make_config(2)));
      let elapsed = started_at.elapsed();

      let latest = config.load();
      println!(
        "arc_swap writer published {} in {:?}",
        config_summary(&latest),
        elapsed
      );
    })
  };

  reader.join().expect("arc_swap reader thread panicked");
  writer.join().expect("arc_swap writer thread panicked");

  let latest = config.load();
  println!("arc_swap final config: {}", config_summary(&latest));
}

fn run_rwlock_example() {
  println!("=== Arc<RwLock<T>>: protect one mutable config with read/write locks ===");

  let config = Arc::new(RwLock::new(make_config(1)));
  let (reader_ready_tx, reader_ready_rx) = mpsc::channel();

  let reader = {
    let config = Arc::clone(&config);

    thread::spawn(move || {
      let snapshot = config.read().expect("rwlock read lock was poisoned");
      println!("rwlock reader got {}", config_summary(&snapshot));

      reader_ready_tx
        .send(())
        .expect("rwlock reader failed to notify writer");

      thread::sleep(Duration::from_millis(200));

      println!(
        "rwlock reader still holds read lock after sleep: {}",
        config_summary(&snapshot)
      );
    })
  };

  let writer = {
    let config = Arc::clone(&config);

    thread::spawn(move || {
      reader_ready_rx
        .recv()
        .expect("rwlock writer failed to wait for reader");

      let started_at = Instant::now();
      let mut locked_config = config.write().expect("rwlock write lock was poisoned");
      let elapsed = started_at.elapsed();

      *locked_config = make_config(2);

      println!(
        "rwlock writer acquired write lock after {:?} and updated {}",
        elapsed,
        config_summary(&locked_config)
      );
    })
  };

  reader.join().expect("rwlock reader thread panicked");
  writer.join().expect("rwlock writer thread panicked");

  let latest = config.read().expect("rwlock read lock was poisoned");
  println!("rwlock final config: {}", config_summary(&latest));
}

fn make_config(version: u64) -> AppConfig {
  AppConfig {
    version,
    feature_enabled: version % 2 == 0,
    endpoint: format!("https://api-v{version}.example.test"),
  }
}

fn config_summary(config: &AppConfig) -> String {
  format!(
    "version={}, feature_enabled={}, endpoint={}",
    config.version, config.feature_enabled, config.endpoint
  )
}
