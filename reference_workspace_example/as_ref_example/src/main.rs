fn is_hello<T: AsRef<str>>(s: T) {
  assert_eq!("hello", s.as_ref());
}

fn add_one<T: AsMut<u64>>(num: &mut T) {
  *num.as_mut() += 1;
}

fn main() {
  println!("Hello, world!");

  let s = "hello";
  is_hello(s);

  let s = "hello".to_string();
  is_hello(s);

  let mut boxed_num = Box::new(0);
  add_one(&mut boxed_num);
  assert_eq!(*boxed_num, 1);
}
