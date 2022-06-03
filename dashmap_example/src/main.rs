use dashmap::DashMap;

use parking_lot::RwLock;
use std::io;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    // println!("Hello, world!");
    let reviews: DashMap<&str, &str> = DashMap::<&str, &str>::new();
    reviews.insert("Veloren", "What a fantastic game!");

    println!("reviews:{:?}", reviews);

    let dict: Arc<DashMap<String, RwLock<u8>>> = Arc::new(DashMap::<String, RwLock<u8>>::new());

    let dict_clone = dict.clone();
    std::thread::spawn(move || dict_clone.insert("a".to_owned(), RwLock::new(1u8)));

    let dict_clone = dict.clone();
    std::thread::spawn(move || dict_clone.insert("b".to_owned(), RwLock::new(2u8)));

    let dict_clone = dict.clone();
    std::thread::spawn(move || {
        let _ = dict_clone.entry("c".to_owned()).or_try_insert_with(|| {
            if 3 > 2 {
                Ok(RwLock::new(3u8))
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "Chunk not found"))
            }
        });
    });

    sleep(Duration::from_millis(20));
    println!("dict: {:?}", dict);
}
