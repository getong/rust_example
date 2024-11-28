use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::StreamExt;

fn main() {
  let counter = Rc::new(RefCell::new(HashMap::<String, usize>::new()));
  let consumer = futures::stream::repeat("foo!".to_string()).take(100);

  futures::executor::block_on(
    consumer
      .map(move |msg| Context::new(msg, counter.clone()))
      .for_each_concurrent(None, |context| {
        async move {
          println!("context:{:?}", context);
          // Pull function value `context` inside Future.

          // Process message...

          // Count messages
          let mut counter = context.counter.borrow_mut();
          *counter.entry(context.message.clone()).or_insert(0) += 1;
          println!("counter:{:?}", counter);
        }
      }),
  )
}

#[derive(Debug)]
struct Context {
  message: String,
  counter: Rc<RefCell<HashMap<String, usize>>>,
}

impl Context {
  fn new(message: String, counter: Rc<RefCell<HashMap<String, usize>>>) -> Self {
    Context { message, counter }
  }
}
