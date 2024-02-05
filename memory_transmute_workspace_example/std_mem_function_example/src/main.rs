use std::mem;

fn main() {
  // println!("Hello, world!");
  let mut v: Vec<String> = Vec::new();
  for i in 101..106 {
    v.push(i.to_string());
  }

  let fifth = v.pop().expect("vector empty!");
  assert_eq!(fifth, "105");

  let second = v.swap_remove(1);
  assert_eq!(second, "102");

  let third = mem::replace(&mut v[2], "substitute".to_string());
  assert_eq!(third, "103");

  assert_eq!(v, vec!["101", "104", "substitute"]);

  swap_function();

  mem_take_example();
}

fn swap_function() {
  let mut x = 5;
  let mut y = 42;

  mem::swap(&mut x, &mut y);

  assert_eq!(42, x);
  assert_eq!(5, y);
}

fn mem_take_example() {
  struct Buffer<T> {
    buf: Vec<T>,
  }
  impl<T> Buffer<T> {
    fn get_and_reset(&mut self) -> Vec<T> {
      mem::take(&mut self.buf)
    }
  }

  let mut buffer = Buffer { buf: vec![0, 1] };
  assert_eq!(buffer.buf.len(), 2);

  assert_eq!(buffer.get_and_reset(), vec![0, 1]);
  assert_eq!(buffer.buf.len(), 0);
}
