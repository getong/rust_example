//! Runnable examples for closure capture, adaptation, and limitations.
//!
//! Run with `cargo run`.  The detailed explanation is in `README.org`.

use std::cell::Cell;

fn main() {
  capture_and_read_data();
  expose_captured_data();
  adapt_to_other_closure_forms();
  demonstrate_fn_traits();
  demonstrate_fn_pointer_limits();
}

/// A closure captures by borrow, by mutable borrow, or by moving ownership.
fn capture_and_read_data() {
  let name = String::from("Rust");
  let greet = || format!("hello, {name}");
  assert_eq!(greet(), "hello, Rust");

  let mut count = 0;
  let mut increment = || {
    count += 1;
    count
  };
  assert_eq!(increment(), 1);
  assert_eq!(increment(), 2);

  let owned = String::from("owned by closure");
  let consume = move || owned;
  assert_eq!(consume(), "owned by closure");
}

/// A closure's captured fields are private compiler-generated fields. Expose
/// the value explicitly by returning it or by providing an accessor closure.
fn expose_captured_data() {
  let (value, get_value) = make_reader(String::from("captured"));
  assert_eq!(get_value(), "captured");
  assert_eq!(value, "captured");

  let state = Cell::new(10);
  let read_state = || state.get();
  assert_eq!(read_state(), 10);
  state.set(20);
  assert_eq!(read_state(), 20);
}

fn make_reader(value: String) -> (String, impl Fn() -> String) {
  let reader_value = value.clone();
  (value, move || reader_value.clone())
}

/// These are adaptations, not casts between two anonymous closure types.
fn adapt_to_other_closure_forms() {
  let captured = 3;
  let original = move |input: i32| input + captured;

  let adapted = adapt_fn(original);
  assert_eq!(adapted(4), 7);

  let boxed: Box<dyn Fn(i32) -> i32> = Box::new(adapted);
  assert_eq!(boxed(5), 8);

  let function = non_capturing_closure_to_fn_pointer();
  assert_eq!(function(4), 16);

  let from_function = |input| function(input);
  assert_eq!(from_function(5), 25);
}

fn adapt_fn<F>(function: F) -> impl Fn(i32) -> i32
where
  F: Fn(i32) -> i32,
{
  move |input| function(input)
}

fn non_capturing_closure_to_fn_pointer() -> fn(i32) -> i32 {
  (|input: i32| input * input) as fn(i32) -> i32
}

/// `Fn` can be called repeatedly, `FnMut` needs exclusive access, and `FnOnce`
/// consumes the closure. The bounds form a capability hierarchy:
/// `Fn` implements `FnMut`, and `FnMut` implements `FnOnce`.
fn demonstrate_fn_traits() {
  let add_one = |input| input + 1;
  assert_eq!(call_repeatedly(add_one, 4), 10);

  let mut total = 0;
  let add_to_total = |input| {
    total += input;
    total
  };
  assert_eq!(call_mutably(add_to_total, 2), 2);

  let text = String::from("consumed");
  let consume = move || text;
  assert_eq!(call_once(consume), "consumed");
}

fn call_repeatedly<F>(function: F, input: i32) -> i32
where
  F: Fn(i32) -> i32,
{
  function(input) + function(input)
}

fn call_mutably<F>(mut function: F, input: i32) -> i32
where
  F: FnMut(i32) -> i32,
{
  function(input)
}

fn call_once<F>(function: F) -> String
where
  F: FnOnce() -> String,
{
  function()
}

fn demonstrate_fn_pointer_limits() {
  fn triple(input: i32) -> i32 {
    input * 3
  }

  let pointer: fn(i32) -> i32 = triple;
  assert_eq!(pointer(4), 12);

  // A function pointer has no environment, so a capturing closure cannot be
  // assigned to `fn(i32) -> i32`. Use `impl Fn` or `Box<dyn Fn>` instead.
}
