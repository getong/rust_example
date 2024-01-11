#![feature(fn_traits)]

fn create_closure() -> impl FnOnce() {
  let name = String::from("john");
  || {
    drop(name);
  }
}

fn main() {
  let a = create_closure();
  a();

  let b = create_closure();
  b.call_once(());
}
