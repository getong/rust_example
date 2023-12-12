fn main() {
  // println!("Hello, world!");
  let mut num = Box::new(5);
  *num.as_mut() += 5;
  println!("num: {:?}", num);
}
