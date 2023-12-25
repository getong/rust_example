#[derive(Debug)]
struct Point {
  x: i32,
  y: i32,
  z: String,
}

fn main() {
  let p = Point {
    x: 10,
    y: 20,
    z: "this is old".to_owned(),
  };
  println!("x is {}, y is {}", p.x, p.y);

  println!("Temporary variable: {:?}", p);

  println!("clone string: {}", p.z.clone().replace("old", "new"));
  assert_eq!("this is new", p.z.clone().replace("old", "new"));
}
