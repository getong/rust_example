use std::sync::{Arc, Condvar, Mutex};
use std::thread;

fn main() {
    // println!("Hello, world!");
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let pair_clone = pair.clone();

    thread::spawn(move || {
        let &(ref lock, ref cvar) = &*pair_clone;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_one();
    });

    let &(ref lock, ref cvar) = &*pair;
    let mut started = lock.lock().unwrap();

    while !*started {
        println!("{}", started); // false
        started = cvar.wait(started).unwrap();
        println!("{}", started); // true
    }
}
