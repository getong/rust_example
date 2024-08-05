fn f1(mut r: impl std::io::Read) {
  let buf: &mut [u8] = &mut [0; 2];
  let _ = r.read(buf);
  println!("read bytes: {:?}", buf);
}

fn main() {
  {
    println!("======= &[u8] =======");
    let bytes: &[u8] = &[1, 2, 3];
    f1(bytes);
    println!("rest-bytes: {:?}", bytes);
  }

  {
    println!("======= &mut &[u8] =======");
    let mut bytes: &[u8] = &[1, 2, 3];
    f1(&mut bytes);
    println!("rest-bytes: {:?}", bytes);
  }

  let arr = [1, 2, 3, 4, 5];
  let vec = vec![1, 2, 3, 4, 5];
  let s1 = &arr[.. 2];
  let s2 = &vec[.. 2];
  println!("s1: {:?}, s2: {:?}", s1, s2);

  // &[T] 和 &[T] 是否相等取决于长度和内容是否相等
  assert_eq!(s1, s2);
  // &[T] 可以和 Vec<T>/[T;n] 比较，也会看长度和内容
  assert_eq!(&arr[..], vec);
  assert_eq!(&vec[..], arr);
}
