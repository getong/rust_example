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

    // Downcast the Any to a MyStruct
    let concrete_instance = any.downcast::<MyStruct>();

    // Match on the result of the downcast
    match concrete_instance {
        Ok(concrete_instance) => {
            concrete_instance.do_something();
        }
        Err(_) => {
            println!("Failed to downcast to MyStruct");
        }
    }
}
