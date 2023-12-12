fn foo() -> i32 {
  0
}

fn bar() {
  println!("Hello World");
}

fn main() {
  // "*const ()" is similar to "const void*" in C/C++.
  let pointer = foo as *const ();
  let function = unsafe { std::mem::transmute::<*const (), fn() -> i32>(pointer) };
  assert_eq!(function(), 0);

  let pointer = bar as *const ();
  let function = unsafe { std::mem::transmute::<*const (), fn() -> ()>(pointer) };

  function();
}
