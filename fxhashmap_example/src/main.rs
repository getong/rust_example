fn main() {
    // println!("Hello, world!");
    use rustc_hash::FxHashMap;
    let mut map: FxHashMap<u32, u32> = FxHashMap::default();
    map.insert(22, 44);

    println!("map:{:?}", map);
}
