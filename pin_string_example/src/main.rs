use std::pin::Pin;

fn main() {
    // println!("Hello, world!");
    let mut string = "Pinned?".to_string();
    let mut pinned: Pin<&mut String> = Pin::new(&mut string);

    pinned.push_str(" Not");
    Pin::into_inner(pinned).push_str(" so much.");

    let new_home = string;
    assert_eq!(new_home, "Pinned? Not so much.");
}
