use crossbeam_channel::{select, unbounded};
use std::time::Duration;

fn main() {
  let (s1, r1) = unbounded::<i32>();
  let (_s2, r2) = unbounded::<i32>();
  s1.send(10).unwrap();

  select! {
      recv(r1) -> msg => println!("r1 > {}", msg.unwrap()),
      recv(r2) -> msg => println!("r2 > {}", msg.unwrap()),
      default(Duration::from_millis(100)) => println!("timed out"),
  }
}
