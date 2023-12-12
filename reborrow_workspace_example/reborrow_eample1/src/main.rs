fn mutate(i: &mut u32) -> &mut u32 {
  *i += 1;
  i
}

fn mutate_twice(i: &mut u32) -> &mut u32 {
  mutate(i);
  mutate(i)
}

fn main() {
  // println!("Hello, world!");
  let mut i = 32;
  mutate_twice(&mut i);
  mutate_twice(&mut i);

  println!("i: {}", i);
}
