use std::collections::HashMap;

fn main() {
    // println!("Hello, world!");
    let mut v = [1, 2, 3];
    let mut iter = v.iter();

    println!("{:?}", iter.next());
    println!("{:?}", iter.next());
    println!("{:?}", iter.next());
    println!("{:?}", iter.next());

    for i in &v {
        println!("i is {:?}", i)
    }

    for i in &mut v {
        *i += 1;
    }

    for i in v {
        println!("i is {:?}", i)
    }

    // Create a HashMap with some key-value pairs
    let mut my_map = HashMap::new();
    my_map.insert("apple", 3);
    my_map.insert("banana", 2);
    my_map.insert("orange", 5);

    // Iterate over the key-value pairs using `iter()`
    for (key, value) in my_map.iter() {
        println!("Key: {}, Value: {}", key, value);
    }

    for (key, value) in &my_map {
        println!("Key: {}, Value: {}", key, value);
    }

    for (key, value) in &mut my_map {
        println!("Key: {}, Value: {}", key, value);
        *value += 1;
    }

    for (key, value) in my_map {
        println!("Key: {}, Value: {}", key, value);
    }
}
