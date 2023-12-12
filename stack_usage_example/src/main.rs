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

  // sort example
  let mut sort_stack = Vec::new();
  sort_stack.push("anteater");
  sort_stack.push("zebra");
  sort_stack.push("tapir");
  sort_stack.push("elephant");
  sort_stack.push("coati");
  sort_stack.push("leopard");
  sort_stack.sort();
  while let Some(animal) = sort_stack.pop() {
    println!("{}", animal);
  }
}
