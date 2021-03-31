use std::vec::Vec;

fn main() {
    let mut vec_stack = Vec::new();
    vec_stack.push("First entry on");
    vec_stack.push("Second entry on");
    vec_stack.push("Third entry on");
    vec_stack.push("Fourth entry on");

    while let Some(top_entry) = vec_stack.pop() {
        println!("{}", top_entry);
    }

    if let Some(another_entry) = vec_stack.pop() {
        println!("Final entry: {}", another_entry);
    } else {
        println!("No entries left");
    }
}
