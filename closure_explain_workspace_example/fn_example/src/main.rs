#![feature(fn_traits)]

fn create_closure() -> impl Fn() {
  let msg = String::from("hello");
  let v: Vec<i32> = vec![1, 2];

  // please note that, move keyword
  move || {
    println!("{}", msg);
    println!("{:?}", v);
  }
}

fn create_closure2() {
  let msg = String::from("hello");

  let my_print = || {
    println!("{}", msg);
  };

  my_print();
  my_print();
}

fn main() {
  let a = create_closure();
  a();
  a();

  a.call(());

  create_closure2();
  create_closure2();
}
