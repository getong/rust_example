#![feature(coroutines, coroutine_trait)]

use std::{
  ops::{Coroutine, CoroutineState},
  pin::Pin,
};

fn main() {
  // Define a coroutine for Fibonacci sequence
  let mut generator = || {
    let mut curr: u64 = 1;
    let mut next: u64 = 1;

    loop {
      let new_next = curr.checked_add(next);

      if let Some(new_next) = new_next {
        curr = next;
        next = new_next;
        yield curr; // Produce the current Fibonacci value
      } else {
        return; // End the coroutine if overflow occurs
      }
    }
  };

  // Resume the coroutine in a loop
  loop {
    match Pin::new(&mut generator).resume(()) {
      CoroutineState::Yielded(v) => println!("{}", v), // Print the yielded Fibonacci value
      CoroutineState::Complete(_) => return,           // Exit the loop when the coroutine is done
    }
  }
}
