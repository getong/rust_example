use hashbrown::HashMap;

fn main() {
    // println!("Hello, world!");
    let mut map = HashMap::new();
    map.insert(1, "one");
    println!("map: {:?}", map);
}
