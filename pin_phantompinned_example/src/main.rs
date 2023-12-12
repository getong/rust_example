// Note that MyData does not implement Clone or Copy
struct MyData(u32);

impl MyData {
  fn print_addr(&self) {
    println!("Address: {:p}", self);
  }
}

use std::{marker::PhantomPinned, pin::Pin};

struct MyData2(u32, PhantomPinned);

impl MyData2 {
  fn print_addr2(self: Pin<&Self>) {
    println!("Address: {:p}", self);
  }
}

fn main() {
  // On the heap
  let x_heap = Box::new(MyData(42));
  x_heap.print_addr();

  // Moved back on the stack
  let x_stack = *x_heap;
  x_stack.print_addr();

  // On the heap
  let x_pinned = Box::pin(MyData2(42, PhantomPinned));
  x_pinned.as_ref().print_addr2();

  // Moved back on the stack
  // let x_unpinned = Pin::into_inner(x_pinned); // FAILS!
  // let x_stack = *x_unpinned;
  // let x_pinned_again = Box::pin(x_stack);
  // x_pinned_again.as_ref().print_addr();
}
