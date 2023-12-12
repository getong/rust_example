use std::rc::Rc;

fn main() {
  let a = Rc::new(1);
  println!("count after creating a, a = {}", Rc::strong_count(&a));
  let b = Rc::clone(&a);
  println!("count after creating b, b = {}", Rc::strong_count(&b));
  println!("count after creating b, a = {}", Rc::strong_count(&a));
  {
    let c = Rc::clone(&a);
    println!("count after creating c, c = {}", Rc::strong_count(&c));
    println!("count after creating c, a = {}", Rc::strong_count(&a));
  }
  println!(
    "count after c goes out of scope, a = {}",
    Rc::strong_count(&a)
  );

  println!(
    "count after c goes out of scope, b = {}",
    Rc::strong_count(&b)
  );
}
