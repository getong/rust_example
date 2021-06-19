use std::cell::RefCell;
use std::rc::Rc;

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
}
