use std::sync::Mutex;

use once_cell::sync::Lazy;

static LOG_FILE: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn get_log_file() -> String {
  LOG_FILE.lock().unwrap().clone()
}

pub fn set_log_file(file: String) {
  *LOG_FILE.lock().unwrap() = file;
}

fn main() {
  // println!("Hello, world!");
  println!("get log file , the string is {}", get_log_file());
  set_log_file("hello".to_string());
  println!("get log file , the string is {}", get_log_file());
}
