macro_rules! four {
  () => {
    1 + 3
  };
}

fn main() {
  // println!("Hello, world!");
  let a = four!();
  println!("a:{}", a);

  let b = four![];
  println!("b:{}", b);

  let c = four! {};
  println!("c:{}", c);
}
