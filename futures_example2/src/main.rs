use futures::executor::block_on;
use futures::join;
use std::{thread, time};

async fn do_something(number: i8) -> i8 {
  println!("number {} is running", number);
  let two_seconds = time::Duration::new(2, 0);
  thread::sleep(two_seconds);
  return 2;
}

fn main() {
  let now = time::Instant::now();
  let future_one = do_something(1);
  let outcome = block_on(future_one);
  println!("time elapsed {:?}", now.elapsed());
  println!("Here is the outcome: {}", outcome);

  let second_outcome = async {
    let future_two = do_something(2);
    let future_three = do_something(3);
    return join!(future_two, future_three);
  };
  let now = time::Instant::now();
  let result = block_on(second_outcome);
  println!("time elapsed {:?}", now.elapsed());
  println!("here is the result: {:?}", result);
}
