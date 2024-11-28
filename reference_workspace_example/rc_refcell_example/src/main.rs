use std::{cell::RefCell, rc::Rc};

fn main() {
  let shared_vec: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
  // Output: []
  println!("{:?}", shared_vec.borrow());

  // {
  let b = Rc::clone(&shared_vec);
  b.borrow_mut().push(1);
  b.borrow_mut().push(2);
  //}

  shared_vec.borrow_mut().push(3);
  // Output: [1, 2, 3]
  println!("{:?}", shared_vec.borrow());

  let data = Rc::new(RefCell::new(42_i32));
  {
    let mut borrowed_data = data.borrow_mut();
    *borrowed_data += 10;
  }

  println!("Value: {}", data.borrow());

  *data.borrow_mut() += 10;

  println!("Value: {}", data.borrow());
}
