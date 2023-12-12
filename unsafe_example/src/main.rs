fn very_trustworthy(shared: &i32) {
  unsafe {
    let mutable = shared as *const i32 as *mut i32;
    *mutable = 20;
  }
}

fn main() {
  println!("Hello, world!");

  let num: u32 = 42;

  let p: *const u32 = &num;

  unsafe {
    assert_eq!(*p, num);
  }

  let i: i32 = 10;

  println!("before changed i is {}", i);
  very_trustworthy(&i);
  println!("after changed i is {}", i);

  drop_and_print();
}

fn drop_and_print() {
  let x = 42;
  let ptr = &x as *const _;
  #[allow(dropping_copy_types)]
  drop(x);
  let y = unsafe { *ptr };
  println!("{:?}", y);
}
