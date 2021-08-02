// mutex_basics.rs
use std::sync::Mutex;
use std::thread;

fn main() {
    let m = Mutex::new(0);
    let c = thread::spawn(move || {
        *m.lock().unwrap() += 1;

        let updated = *m.lock().unwrap();
        updated
    });
    let updated = c.join().unwrap();
    println!("{:?}", updated);

    try_lock();
}

fn try_lock() {
    let my_mutex = Mutex::new(5);
    let _mutex_changer = my_mutex.lock().unwrap();
    let other_mutex_changer = my_mutex.try_lock();

    if let Ok(value) = other_mutex_changer {
        println!("The MutexGuard has: {}", value)
    } else {
        println!("Didn't get the lock")
    }
}
