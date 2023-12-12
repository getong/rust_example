const fn foo(num: i32) -> i32 {
  if 3 > 2 {
    2
  } else {
    num * num
  }
}

fn main() {
  println!("Hello, world!");
  println!("foo(3): {}", foo(3));
}
