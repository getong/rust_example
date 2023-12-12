use std::collections::VecDeque;

fn pal_checker(pal: &str) -> bool {
  let mut d = VecDeque::with_capacity(pal.len());
  for c in pal.chars() {
    let _r = d.push_back(c);
  }

  let mut is_pal = true;

  while d.len() > 1 && is_pal {
    let head = d.pop_front();
    let tail = d.pop_back();
    if head != tail {
      is_pal = false;
    }
  }
  is_pal
}

fn main() {
  // println!("Hello, world!");
  let pal = "rustsur";
  let is_pal = pal_checker(pal);
  println!("{} is palindrome string: {}", pal, is_pal);

  let pal = "rustsu";
  let is_pal = pal_checker(pal);
  println!("{} is palindrome string: {}", pal, is_pal);
}
