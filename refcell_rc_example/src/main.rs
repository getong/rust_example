use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
struct Data {
  value: i32,
}

fn main() {
  // Create a RefCell<Rc<T>> holding the data
  let data = RefCell::new(Rc::new(Data { value: 42 }));
  {
    // Clone the Rc<T> and update the value
    let mut borrowed_data = data.borrow_mut();

    // if let Some(mut_data) = Rc::get_mut(&mut borrowed_data) {
    //     mut_data.value = 46;
    // } else {
    //     println!("no data");
    // }
    // this will not fail, get_mut() might fail
    let mut_data = Rc::make_mut(&mut borrowed_data);
    mut_data.value = 46;
  }

  // Access the data and print its value
  let borrowed_data = data.borrow();
  let x_ptr = Rc::into_raw(borrowed_data.into());
  unsafe {
    println!("Value: {:?}", (*x_ptr).value);
  }
}
