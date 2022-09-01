use std::sync::{mpsc::channel, Arc, Mutex};

pub fn main() {
    mutex_channel_example();
    normal_channel_example();
}

fn mutex_channel_example() {
    let (tx, rx) = channel();

    let x = Arc::new(Mutex::new(tx));

    std::thread::spawn(move || {
        x.lock().unwrap().send(4u8).unwrap();
    });

    dbg!(rx.recv().unwrap());
}

fn normal_channel_example() {
    let (tx, rx) = channel();

    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        _ = tx_clone.send(4u8);
    });

    dbg!(rx.recv().unwrap());
}
