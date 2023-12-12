#![allow(unused)]
struct TypeWithDestructor(i32);

impl Drop for TypeWithDestructor {
  fn drop(&mut self) {
    println!("Dropped. Held {}.", self.0);
  }
}

const ZERO_WITH_DESTRUCTOR: TypeWithDestructor = TypeWithDestructor(0);

fn create_and_drop_zero_with_destructor() {
  let mut x = ZERO_WITH_DESTRUCTOR;
  // x gets dropped at end of function, calling drop.
  // prints "Dropped. Held 0.".
  x.0 = 3;
  println!(
    "x is at {:?}",
    &mut x as *const TypeWithDestructor as *const ()
  )
}

fn main() {
  create_and_drop_zero_with_destructor();
  println!("hello world");
  create_and_drop_zero_with_destructor();

  let mut x = ZERO_WITH_DESTRUCTOR;
  // x gets dropped at end of function, calling drop.
  // prints "Dropped. Held 0.".
  x.0 = 3;
  println!(
    "x is at {:?}",
    &mut x as *const TypeWithDestructor as *const ()
  );
}
