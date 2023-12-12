use std::{
  sync::Arc,
  thread::{self, JoinHandle},
};

fn main() {
  // println!("Hello, world!");

  // Observe that we now wrap the string in an `Arc`.
  let username: Arc<String> = Arc::new("hello world".to_owned());

  // Spawn ten threads again
  let handles: Vec<_> = (0..10)
    .map(|id| {
      // Here, we are explicitly cloning the smart pointer,
      // not the `String` itself. Note that it is possible to
      // write `username.clone()` instead, but that may be ambiguous
      // to readers of this code. It is best to explicitly
      // invoke the `Clone` implementation for `Arc`.
      let cloned = Arc::clone(&username);

      // We now move the cloned smart pointer to the thread.
      thread::spawn(move || {
        println!("Hello {} from Thread #{}!", cloned, id);
      })
    })
    .collect();

  // Join all the threads again
  handles
    .into_iter()
    .try_for_each(JoinHandle::join)
    .expect("failed to join all threads");
}
