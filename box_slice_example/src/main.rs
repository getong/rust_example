use std::{mem::size_of_val, ops::Deref};

fn main() {
  let mut v1 = vec![1, 2, 3, 4];
  v1.push(5);
  assert_eq!(v1.capacity(), 8);

  // 从 Vec<T> 转换成 Box<[T]>，此时会丢弃多余的 capacity
  let b1 = v1.into_boxed_slice();
  let mut b2 = b1.clone();

  let v2 = b1.into_vec();
  assert_eq!(v2.capacity(), 5);

  assert!(b2.deref() == v2);

  // Box<[T]> 可以更改其内部数据，但无法 push
  b2[0] = 2;
  // b2.push(6);
  println!("b2: {:?}", b2);

  // 注意 Box<[T]> 和 Box<[T; n]> 并不相同
  let b3 = Box::new([2, 2, 3, 4, 5]);
  println!("b3: {:?}", b3);

  // b2 和 b3 相等，但 b3.deref() 和 v2 无法比较
  assert!(b2 == b3);
  // assert!(b3.deref() == v2);

  another_box_slice_exmaple();
}

// copy from [The curse of strong typing](https://fasterthanli.me/articles/the-curse-of-strong-typing)
fn another_box_slice_exmaple() {
  let bbox: Box<[u8]> = Box::new([1, 2, 3, 4, 5]);
  let slice = &bbox[1 .. 4];
  dbg!(size_of_val(&bbox));
  dbg!(size_of_val(&slice));
  print_byte_slice(slice);
}

fn print_byte_slice(slice: &[u8]) {
  println!("{slice:?}");
}
