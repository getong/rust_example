#![feature(fn_traits)]

fn main() {
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
