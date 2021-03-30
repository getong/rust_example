// arc_mutex.rs
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let vec = Arc::new(Mutex::new(vec![]));
    let mut childs = vec![];
    for i in 0..5 {
        let v = vec.clone();
        let t = thread::spawn(move || {
            let mut v = v.lock().unwrap();
            v.push(i);
        });
        childs.push(t);
    }

    for c in childs {
        c.join().unwrap();
    }

    println!("{:?}", vec);

    let data = Arc::new(Mutex::new(0));
    for _ in 0..15 {
        let data = Arc::clone(&data);
        thread::spawn(move || {
            let mut data = data.lock().unwrap();
            *data += 1;
            if *data == 15 {
                return;
            }
        });
    }
    println!("data: {}", data.lock().unwrap());
}
