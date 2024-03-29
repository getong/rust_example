use std::panic::{self, AssertUnwindSafe};

fn main() {
  let mut variable = 4;
  let mut s = "hello".to_string();

  // This code will not compile because the closure captures `&mut variable`
  // which is not considered unwind safe by default.

  // panic::catch_unwind(|| {
  //     variable += 3;
  // });

  // This, however, will compile due to the `AssertUnwindSafe` wrapper
  let result = panic::catch_unwind(AssertUnwindSafe(|| {
    variable += 3;
    s += " world";
  }));

  if result.is_ok() {
    println!("variable is {:?}, s: {:?}", variable, s);
  }
}
