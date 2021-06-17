use dashmap::DashMap;

fn main() {
    // println!("Hello, world!");
    let reviews: DashMap<&str, &str> = DashMap::<&str, &str>::new();
    reviews.insert("Veloren", "What a fantastic game!");

    println!("reviews:{:?}", reviews);
}
