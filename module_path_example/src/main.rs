macro_rules! get_module_name {
  () => {{
    // Using the module_path! macro to get the current module's path
    module_path!()
  }};
}

fn main() {
  // Using the get_module_name! macro to get the module name
  let module_name = get_module_name!();

  // Printing the module name
  println!("The current module is: {}", module_name);
}
