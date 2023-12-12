use std::sync::atomic::{AtomicU8, Ordering};

static LOG_LEVEL: AtomicU8 = AtomicU8::new(0);

pub fn get_log_level() -> u8 {
  LOG_LEVEL.load(Ordering::Relaxed)
}

pub fn set_log_level(level: u8) {
  LOG_LEVEL.store(level, Ordering::Relaxed);
}

fn change_variable_one() {
  println!("one before set level is {}", get_log_level());
  std::thread::spawn(|| set_log_level(1));
  // std::thread::sleep(std::time::Duration::from_secs(1));
  println!("one after set level is {}", get_log_level());
}

fn change_variable_two() {
  println!("two before set level is {}", get_log_level());
  let handler = std::thread::spawn(|| set_log_level(2));
  handler.join().unwrap();
  println!("two after set level is {}", get_log_level());
}

fn main() {
  change_variable_one();
  change_variable_two();
}
