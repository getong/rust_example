// Define functions that you want to call by string name
fn function1() {
  println!("Calling Function 1");
}

fn function2() {
  println!("Calling Function 2");
}

fn function3() {
  println!("Calling Function 3");
}

// Macro to generate a match statement based on the function name
macro_rules! call_function_by_name {
  ($name:expr) => {
    match $name {
      "function1" => function1(),
      "function2" => function2(),
      "function3" => function3(),
      _ => println!("Function not found"),
    }
  };
}

fn main() {
  // Example usage
  call_function_by_name!("function1");
  call_function_by_name!("function2");
  call_function_by_name!("function3");
  call_function_by_name!("unknown_function");
}
