use std::{rc::Rc, sync::Arc};

#[derive(Debug)]
struct User {
  name: Rc<String>,
}

// wrong impl here
unsafe impl Send for User {}
unsafe impl Sync for User {}

fn main() {
  let foo = Arc::new(User {
    name: Rc::new(String::from("drogus")),
  });

  let foo_clone = foo.clone();
  std::thread::spawn(move || {
    _ = foo_clone.name.clone();
  });

  let foo_clone = foo.clone();
  std::thread::spawn(move || {
    _ = foo_clone.name.clone();
  });
}
