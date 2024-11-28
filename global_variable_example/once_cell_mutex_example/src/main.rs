use std::sync::Mutex;

use once_cell::sync::OnceCell;

static LOG_FILE: OnceCell<Mutex<String>> = OnceCell::new();

fn ensure_log_file() -> &'static Mutex<String> {
  LOG_FILE.get_or_init(|| Mutex::new(String::new()))
}

pub fn get_log_file() -> String {
  ensure_log_file().lock().unwrap().clone()
}

pub fn set_log_file(file: String) {
  *ensure_log_file().lock().unwrap() = file;
}

fn main() {
  // println!("Hello, world!");
  println!("get_log_file, the string: {:?}", get_log_file());
  set_log_file("hello".to_string());
  println!("get_log_file, the string: {:?}", get_log_file());
}
