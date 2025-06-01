use std::{
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use parking_lot::{Condvar, Mutex, RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};

#[derive(Default, Debug)]
pub struct Payload {
  pub value: u32,
}

#[derive(Default, Debug)]
pub struct Scope {
  data: RwLock<Vec<Mutex<Option<Payload>>>>,
}

impl Scope {
  pub fn set(&self, pos: usize, val: Payload) {
    let data = self.data.upgradable_read();
    if data.len() <= pos {
      // need to resize the table
      let mut wdata = RwLockUpgradableReadGuard::upgrade(data);
      wdata.resize_with(pos + 1, Default::default);
      let data = RwLockWriteGuard::downgrade(wdata);
      *data[pos].lock() = Some(val);
    } else {
      *data[pos].lock() = Some(val);
    }
  }

  pub fn into_data(self) -> Vec<Option<Payload>> {
    self
      .data
      .into_inner()
      .into_iter()
      .map(Mutex::into_inner)
      .collect()
  }

  pub fn into_data_vec(self) -> Vec<Mutex<Option<Payload>>> {
    self.data.into_inner()
  }

  pub fn optimize_set(&self, pos: usize, val: Payload) {
    let mut data = self.data.read();
    if data.len() <= pos {
      // "upgrade" the lock
      drop(data);
      let mut wdata = self.data.write();
      // check that someone else hasn't resized the table in the meantime
      if wdata.len() <= pos {
        wdata.resize_with(pos + 1, Default::default);
      }
      // now "downgrade" it back again
      drop(wdata);
      data = self.data.read();
    }
    *data[pos].lock() = Some(val);
  }

  pub fn demonstrate_fairness() {
    println!("\n=== Demonstrating Fairness ===");

    // This example shows how parking_lot provides fair queuing
    let data = Arc::new(Mutex::new(0u64));
    let mut handles = vec![];

    // Spawn multiple threads that will compete for the lock
    for i in 0 .. 5 {
      let data = Arc::clone(&data);
      let handle = thread::spawn(move || {
        for _ in 0 .. 10 {
          let mut num = data.lock();
          let old_val = *num;
          // Simulate some work
          thread::sleep(Duration::from_millis(1));
          *num = old_val + 1;
          println!("Thread {} incremented to {}", i, *num);
        }
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }

    println!("Final value: {}", *data.lock());
  }

  pub fn demonstrate_performance() {
    println!("\n=== Performance Comparison ===");

    let iterations = 1000000;
    let data = Arc::new(Mutex::new(0u64));

    // Test parking_lot Mutex performance
    let start = Instant::now();
    {
      let mut handles = vec![];
      for _ in 0 .. 4 {
        let data = Arc::clone(&data);
        let handle = thread::spawn(move || {
          for _ in 0 .. (iterations / 4) {
            let mut num = data.lock();
            *num += 1;
          }
        });
        handles.push(handle);
      }

      for handle in handles {
        handle.join().unwrap();
      }
    }
    let parking_lot_time = start.elapsed();

    println!(
      "parking_lot Mutex: {:?} for {} operations",
      parking_lot_time, iterations
    );
    println!("Final value: {}", *data.lock());
  }

  pub fn demonstrate_condvar() {
    println!("\n=== Condvar Example ===");

    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let pair2 = Arc::clone(&pair);

    // Spawn a thread that will signal the condition variable
    thread::spawn(move || {
      thread::sleep(Duration::from_millis(1000));
      let (lock, cvar) = &*pair2;
      let mut started = lock.lock();
      *started = true;
      cvar.notify_one();
      println!("Signaled!");
    });

    // Wait for the signal
    let (lock, cvar) = &*pair;
    let mut started = lock.lock();
    while !*started {
      cvar.wait(&mut started);
    }
    println!("Received signal!");
  }

  pub fn demonstrate_rwlock_features() {
    println!("\n=== RwLock Advanced Features ===");

    let data = Arc::new(RwLock::new(vec![1, 2, 3, 4, 5]));

    // Demonstrate upgradable read lock
    {
      let upgradable = data.upgradable_read();
      println!("Current data: {:?}", *upgradable);

      if upgradable.len() < 10 {
        let mut write_lock = RwLockUpgradableReadGuard::upgrade(upgradable);
        write_lock.push(6);
        write_lock.push(7);
        println!("Added elements");
      }
    }

    // Multiple readers can coexist
    let mut handles = vec![];
    for i in 0 .. 3 {
      let data = Arc::clone(&data);
      let handle = thread::spawn(move || {
        let reader = data.read();
        println!("Reader {}: {:?}", i, *reader);
        thread::sleep(Duration::from_millis(100));
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }

  pub fn demonstrate_timeout_features() {
    println!("\n=== Timeout Features ===");

    let mutex = Arc::new(Mutex::new(42));
    let mutex_clone = Arc::clone(&mutex);

    // Thread that holds the lock for a while
    let handle = thread::spawn(move || {
      let _guard = mutex_clone.lock();
      println!("Background thread acquired lock");
      thread::sleep(Duration::from_millis(2000));
      println!("Background thread releasing lock");
    });

    // Give the other thread time to acquire the lock
    thread::sleep(Duration::from_millis(100));

    // Try to acquire with timeout
    match mutex.try_lock_for(Duration::from_millis(500)) {
      Some(guard) => println!("Acquired lock with timeout: {}", *guard),
      None => println!("Failed to acquire lock within timeout"),
    }

    // Try without blocking
    match mutex.try_lock() {
      Some(guard) => println!("Acquired lock immediately: {}", *guard),
      None => println!("Lock is currently held"),
    }

    // Wait for background thread to finish
    handle.join().unwrap();

    // Now we should be able to acquire the lock
    match mutex.try_lock() {
      Some(guard) => println!("Finally acquired lock: {}", *guard),
      None => println!("Still couldn't acquire lock"),
    };
  }
}

#[derive(Debug, Clone)]
pub enum Load {
  Local(String),
  Remote(String),
  Cache(String),
}

#[derive(Default, Debug)]
pub struct LoadManager {
  load: RwLock<Vec<Option<Load>>>,
}

impl LoadManager {
  pub fn get_or_init(&self, index: usize, key: &str) -> String {
    // First check if the data exists
    {
      let data = self.load.read();
      if let Some(Some(Load::Local(load))) = data.get(index) {
        // do a bunch of stuff with `load`
        println!("Found local load: {}", load);
        return format!("processed: {}", load);
      }
    } // Release read lock before trying to write

    // If not found, initialize
    self.init_for(index, key);
    format!("initialized for key: {}", key)
  }

  fn init_for(&self, index: usize, key: &str) {
    let mut data = self.load.write();
    if data.len() <= index {
      data.resize_with(index + 1, Default::default);
    }
    data[index] = Some(Load::Local(format!("local_{}", key)));
    println!("Initialized load at index {} with key {}", index, key);
  }

  pub fn demonstrate_conditional_pattern() {
    println!("\n=== Conditional Load Pattern ===");
    println!("Starting conditional pattern demonstration...");

    let manager = LoadManager::default();

    // First call - will initialize
    println!("Making first call...");
    let result1 = manager.get_or_init(0, "config");
    println!("Result 1: {}", result1);

    // Second call - will find existing
    println!("Making second call...");
    let result2 = manager.get_or_init(0, "config");
    println!("Result 2: {}", result2);

    // Different index - will initialize again
    println!("Making third call...");
    let result3 = manager.get_or_init(1, "settings");
    println!("Result 3: {}", result3);

    // Demonstrate the pattern with different load types
    println!("Adding additional load types...");
    {
      let mut data = manager.load.write();
      data.push(Some(Load::Remote("remote_data".to_string())));
      data.push(Some(Load::Cache("cached_data".to_string())));
    }

    println!("Final state: {:?}", *manager.load.read());
    println!("Conditional pattern demonstration completed!");
  }

  pub fn demonstrate_advanced_patterns() {
    println!("\n=== Advanced parking_lot Patterns ===");

    // Demonstrate reader-writer lock with multiple concurrent readers
    let shared_data = Arc::new(RwLock::new(vec![1, 2, 3, 4, 5]));
    let mut handles = vec![];

    // Multiple readers
    for i in 0 .. 3 {
      let data = Arc::clone(&shared_data);
      let handle = thread::spawn(move || {
        let reader = data.read();
        println!("Reader {} sees: {:?}", i, *reader);
        thread::sleep(Duration::from_millis(50));
        let sum: i32 = reader.iter().sum();
        println!("Reader {} calculated sum: {}", i, sum);
      });
      handles.push(handle);
    }

    // One writer (will wait for readers to finish)
    let data = Arc::clone(&shared_data);
    let writer_handle = thread::spawn(move || {
      thread::sleep(Duration::from_millis(25)); // Let readers start first
      let mut writer = data.write();
      println!("Writer modifying data...");
      writer.push(6);
      writer.push(7);
      println!("Writer finished: {:?}", *writer);
    });
    handles.push(writer_handle);

    for handle in handles {
      handle.join().unwrap();
    }

    println!("Final data: {:?}", *shared_data.read());
  }
}

fn main() {
  println!("=== Parking Lot Examples from Fly.io Blog ===");

  // Original examples
  let scope = Scope::default();
  scope.optimize_set(0, Payload { value: 0 });
  scope.optimize_set(1, Payload { value: 1 });
  println!("scope : {:?}", scope);

  // New examples from the blog post
  Scope::demonstrate_fairness();
  Scope::demonstrate_performance();
  Scope::demonstrate_condvar();
  Scope::demonstrate_rwlock_features();
  Scope::demonstrate_timeout_features();

  // New conditional pattern example
  LoadManager::demonstrate_conditional_pattern();
  LoadManager::demonstrate_advanced_patterns();

  println!("\n=== Memory Footprint Comparison ===");
  println!(
    "std::sync::Mutex size: {} bytes",
    std::mem::size_of::<std::sync::Mutex<i32>>()
  );
  println!(
    "parking_lot::Mutex size: {} bytes",
    std::mem::size_of::<parking_lot::Mutex<i32>>()
  );
  println!(
    "std::sync::RwLock size: {} bytes",
    std::mem::size_of::<std::sync::RwLock<i32>>()
  );
  println!(
    "parking_lot::RwLock size: {} bytes",
    std::mem::size_of::<parking_lot::RwLock<i32>>()
  );
}
