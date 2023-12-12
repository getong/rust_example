fn main() {
  // a is Copy
  let mut a = 1;
  // b is not Copy
  let b = "hello".to_owned();
  let c: Box<dyn FnMut() + 'static> = Box::new(move || {
    println!("a: {}", a); // error, borrowed value does not live long enough
    a += 3;
    println!("b :{}", b);
  });
  println!("a in the main :{}", a);

  // can not use b here
  // println!("b in the main :{}", b);

  let mut d = c;
  d();
  a += 1;
  println!("a in the main :{}", a);
  d();
}
