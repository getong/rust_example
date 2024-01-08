use std::time::Duration;
use tqdm::tqdm;
use std::thread;

fn main() {
  // println!("Hello, world!");
  let threads: Vec<_> = [200, 400, 100].iter().map(|its| {
    std::thread::spawn(move || {
      for _ in tqdm(0..*its) {
        thread::sleep(Duration::from_millis(10));
      }
    })
  })
    .collect();

  for handle in threads {
    handle.join().unwrap();
  }
}
