use std::rc::Rc;

fn main() {
  // println!("Hello, world!");

  let mut x = Rc::new(3);
  *Rc::get_mut(&mut x).unwrap() = 4;
  assert_eq!(*x, 4);

  let y = Rc::clone(&x);
  println!("_y : {:?}", y);
  assert!(Rc::get_mut(&mut x).is_none());
  println!("x : {:?}", *x);
  // can not call  get_mut() method again
  // *Rc::get_mut(&mut x).unwrap() = 5;
  // assert_eq!(*x, 5);

  let name = Rc::new(String::from("main"));
  let ext = Rc::new(String::from("rs"));

  for _ in 0 .. 3 {
    println!("name: {:?}, ext: {:?}", name.clone(), ext.clone());
  }
}
