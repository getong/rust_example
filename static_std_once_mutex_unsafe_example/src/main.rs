use std::mem::MaybeUninit;
use std::sync::{Mutex, Once};

static mut LOG_FILE: MaybeUninit<Mutex<String>> = MaybeUninit::uninit();
static LOG_FILE_ONCE: Once = Once::new();

fn ensure_log_file() -> &'static Mutex<String> {
    // Safety: initializing the variable is only done once, and reading is
    // possible only after initialization.
    unsafe {
        LOG_FILE_ONCE.call_once(|| {
            LOG_FILE.write(Mutex::new(String::new()));
        });
        // We've initialized it at this point, so it's safe to return the reference.
        LOG_FILE.assume_init_ref()
    }
}

pub fn get_log_file() -> String {
    ensure_log_file().lock().unwrap().clone()
}

pub fn set_log_file(file: String) {
    *ensure_log_file().lock().unwrap() = file;
}

fn main() {
    // println!("Hello, world!");
    println!("get log file is {:?}", get_log_file());
    set_log_file("hello".to_string());
    println!("get log file is {:?}", get_log_file());
}
