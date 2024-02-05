fn check(s: &str) {
  println!("check finished {}", s);
}

fn main() {
  // println!("Hello, world!");
  let a = Some(String::from("jack"));

  let b: Option<&str> = a.as_deref();

  check(b.unwrap());

  println!("{:?}", a);
}
