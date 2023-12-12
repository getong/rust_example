use std::cell::Cell;

#[derive(Debug)]
struct Point {
  x: Cell<i32>,
  y: i32,
}

fn main() {
  let x = Cell::new(1);
  let y = &x;
  let z = &x;
  x.set(2);
  y.set(3);
  z.set(4);
  println!("{:?}", x);

  let p = Point {
    x: Cell::new(1),
    y: 2,
  };
  let p1 = &p;
  let p2 = &p;
  p1.x.set(3);
  p2.x.set(4);

  println!("{:?}", p);
}
