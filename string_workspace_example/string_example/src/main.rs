use std::io::{self, BufRead};
use std::mem;

fn main() {
    // println!("Hello, world!");
    let a: String = "Hello".to_string();
    let b = String::from("Hello");
    let c = "World".to_owned();
    let d = c.clone();
    println!("a: {}, b: {}, c: {}, d: {}", a, b, c, d);

    let mut empty_string = String::new();
    let empty_string_with_capacity = String::with_capacity(50);
    let string_from_bytestring: String =
        String::from_utf8(vec![82, 85, 83, 84]).expect("Creating String from bytestring failed");
    println!("Length of the empty string is {}", empty_string.len());
    println!(
        "Length of the empty string with capacity is {}",
        empty_string_with_capacity.len()
    );
    println!(
        "Length of the string from a bytestring is {}",
        string_from_bytestring.len()
    );
    println!("Bytestring says {}", string_from_bytestring);
    empty_string.push('1');
    println!("1) Empty string now contains {}", empty_string);
    empty_string.push_str("2345");
    println!("2) Empty string now contains {}", empty_string);

    println!(
        "Length of the previously empty string is now {}",
        empty_string.len()
    );

    let mut lang = String::from("rust");
    let rust1 = add_version(&mut lang);
    println!("{:?}", rust1);
    let rust2 = add_lang(&mut lang);
    println!("{:?}", rust2);

    // len() and .chars().count()
    println!("{}", "a".len()); // .len() gives the size in bytes
    println!("{}", "ß".len());
    println!("{}", "国".len());
    println!("{}", "𓅱".len());

    let slice = "Hello!";
    println!(
        "Slice is {} bytes and also {} characters.",
        slice.len(),
        slice.chars().count()
    );
    let slice2 = "안녕!";
    println!(
        "Slice2 is {} bytes but only {} characters.",
        slice2.len(),
        slice2.chars().count()
    );

    let s = from_raw_parts();
    println!("from raw parts: {}", s);

    let s = char_to_string();
    println!("char to string : {}", s);

    let s = vec_to_string();
    println!("vec to string : {}", s);

    println!("enter echo word:");
    let s = buffer_to_string_line();
    println!("buffer to string line : {}", s);
}

fn add_version(s: &mut String) -> String {
    s.push_str(" 2019!!");
    s.to_string()
}

fn add_lang(s: &mut String) -> String {
    s.push_str(" lang.");
    s.to_string()
}

fn from_raw_parts() -> String {
    let story = String::from("Once upon a time...");

    // Prevent automatically dropping the String's data
    let mut story = mem::ManuallyDrop::new(story);

    let ptr = story.as_mut_ptr();
    let len = story.len();
    let capacity = story.capacity();

    // story has nineteen bytes
    assert_eq!(19, len);

    // We can re-build a String out of ptr, len, and capacity. This is all
    // unsafe because we are responsible for making sure the components are
    // valid:
    let s = unsafe { String::from_raw_parts(ptr, len, capacity) };
    s
}

fn char_to_string() -> String {
    let ch = 'c';
    ch.to_string()
}

fn vec_to_string() -> String {
    let hello_world = vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
    // We know it is valid sequence, so we can use unwrap
    String::from_utf8(hello_world).unwrap()
}

fn buffer_to_string_line() -> String {
    let mut line = String::new();
    let stdin = io::stdin();
    stdin.lock().read_line(&mut line).unwrap();
    line
}
