use std::mem::size_of;

// copy from [Exploring Strings in Rust](https://betterprogramming.pub/strings-in-rust-28c08a2d3130)

fn main() {
    assert_eq!(size_of::<Box<str>>(), 16); // 16
    assert_eq!(size_of::<String>(), 24); // 24

    let string_a: Box<str> = String::from("banana").into_boxed_str();
    // or
    let string_b: Box<str> = Box::from("banana");
    // from implementation will yield the same result.

    println!("string_a: {:?}\nstring_b: {:?}", string_a, string_b);
}
