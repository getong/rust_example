#![feature(fn_traits)]
/*
struct MyClosure {
    i: &mut i32,
}

impl FnMut for Myclosure {
    fn call_mut(&mut self) {
        *self.i += 1;
    }
}

*/

fn main() {
  // println!("Hello, world!");

  let mut i: i32 = 0;
  let mut f = || {
    i += 1;
  };

  f();
  f();
  f.call_mut(());

  println!("{}", i);

  let mut j: i32 = 0;
  let mut f = |add_num: i32| {
    j += add_num;
  };

  f(1);
  f(1);
  f(1);
  f.call_mut((1,));
  println!("{}", j);
}
