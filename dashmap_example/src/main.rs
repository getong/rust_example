use dashmap::DashMap;

use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    // println!("Hello, world!");
    let reviews: DashMap<&str, &str> = DashMap::<&str, &str>::new();
    reviews.insert("Veloren", "What a fantastic game!");

    println!("reviews:{:?}", reviews);

    let dict: Arc<DashMap<String, u8>> = Arc::new(DashMap::<String, u8>::new());

    let dict_clone = dict.clone();
    std::thread::spawn(move || dict_clone.insert("a".to_owned(), 1u8));

    let dict_clone = dict.clone();
    std::thread::spawn(move || dict_clone.insert("b".to_owned(), 2u8));

    sleep(Duration::from_millis(10));
    println!("dict: {:?}", dict);
}
