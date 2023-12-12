pub static STATIC_STR: &str = "testing";

mod my_mod {
  use crate::STATIC_STR;

  pub fn test_fn() {
    println!("Here it is: {}", STATIC_STR);
  }
}

fn main() {
  my_mod::test_fn();

  println!("Here it is: {}", STATIC_STR);
}

// copy from https://www.reddit.com/r/rust/comments/a4lnxg/how_to_access_a_mainrs_static_variable_from_a/
