use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref LOG_FILE: Mutex<String> = Mutex::new(String::new());
}

pub fn get_log_file() -> String {
    LOG_FILE.lock().unwrap().clone()
}

pub fn set_log_file(file: String) {
    *LOG_FILE.lock().unwrap() = file;
}

fn main() {
    // println!("Hello, world!");
    println!("get_log_file, the string: {:?}", get_log_file());
    set_log_file("hello".to_string());
    println!("get_log_file, the string: {:?}", get_log_file());
}
