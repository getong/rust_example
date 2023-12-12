mod queue;

use crate::queue::Queue;

fn hot_potato(names: Vec<&str>, num: usize) -> &str {
  let mut q = Queue::new(names.len());
  for name in names {
    let _nn = q.enqueue(name);
  }

  while q.size() > 1 {
    for _i in 0..num {
      let name = q.dequeue().unwrap();
      let _nn = q.enqueue(name);
    }
    let _nn = q.dequeue();
  }
  q.dequeue().unwrap()
}

fn main() {
  // println!("Hello, world!");
  let name = vec!["Shieber", "David", "Susan", "Jane", "Kew", "Brad"];
  let rem = hot_potato(name, 8);
  println!("The left person is {}", rem);
}
