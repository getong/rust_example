use std::any::Any;

trait MyTrait {
  fn do_something(&self);
}

struct MyStruct {
  data: i32,
}

impl MyTrait for MyStruct {
  fn do_something(&self) {
    println!("Data: {}", self.data);
  }
}

fn main() {
  let my_instance: Box<dyn MyTrait> = Box::new(MyStruct { data: 42 });

  // Store the trait object in an Any
  let any: Box<dyn Any> = Box::new(my_instance);

  // Attempt to downcast the Any to Box<dyn MyTrait>
  if let Ok(concrete_instance) = any.downcast::<Box<dyn MyTrait>>() {
    concrete_instance.do_something();
  } else {
    println!("Failed to downcast to Box<dyn MyTrait>");
  }
}
