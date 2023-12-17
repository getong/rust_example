use std::{fmt, ops::Deref};

// Display trait does not impl for Vec<String>
// so need to define a newtype for it.
struct Wrapper(Vec<String>);

impl fmt::Display for Wrapper {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "[{}]", self.0.join(", "))
  }
}

impl Deref for Wrapper {
  type Target = Vec<String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

fn main() {
  let w = Wrapper(vec![String::from("hello"), String::from("world")]);
  println!("w = {}", w);
  println!(
    "wrapper can use vec function join, join result is: {}",
    w.join(",")
  );

  let string_list: Vec<String> = vec!["hello".to_string(), "world".to_string()];
  // we can not use {} here
  println!("the string_list is {:?}", string_list);

  let wrapper = Wrapper(string_list);
  // we can use {} here
  println!("wrapper is {}", wrapper);
  // use deref here
  println!("wrapper deref is {:?}", *wrapper);
}
