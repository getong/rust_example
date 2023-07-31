use std::collections::HashMap;
use std::iter::IntoIterator;

struct Person {
    name: String,
    age: u8,
}

impl IntoIterator for Person {
    type Item = (String, u8);
    type IntoIter = std::option::IntoIter<(String, u8)>;

    fn into_iter(self) -> Self::IntoIter {
        Some((self.name, self.age)).into_iter()
    }
}

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

    let person = Person {
        name: "John Doe".to_string(),
        age: 30,
    };

    // let mut iter = person.into_iter();

    // // Iterate over the names and print them out.
    // while let Some((name, age)) = iter.next() {
    //     println!("The name is: {}, age: {}", name, age);
    // }
    for (name, age) in person {
        println!("name:{:?}, age:{:?}", name, age)
    }
}
