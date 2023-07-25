use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

struct SharedData {
    counter: i32,
}

static SHARED_DATA: OnceCell<Arc<Mutex<SharedData>>> = OnceCell::new();

fn main() {
    SHARED_DATA.get_or_init(|| Arc::new(Mutex::new(SharedData { counter: 0 })));

    let data = SHARED_DATA.clone();

    if let Some(data_mutex) = data.get() {
        if let Ok(mut data_lock) = data_mutex.lock() {
            data_lock.counter += 1;

            println!("data_lock.counter:{}", data_lock.counter);
        }
    }
}
