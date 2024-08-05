use pipeliner::Pipeline;

fn main() {
  // println!("Hello, world!");
  for result in (0 .. 100).with_threads(10).map(|x| x + 1) {
    println!("result: {}", result);
  }

  // You might want a high number of threads for high-latency work:
  let results = (0 .. 100)
    .with_threads(50)
    .map(|x| {
      x + 1 // Let's pretend this is high latency. (ex: network access)
    })
    // But you might want lower thread usage for cpu-bound work:
    .with_threads(4)
    .out_buffer(100)
    .map(|x| {
      x * x // ow my CPUs :p
    });
  for result in results {
    println!("result: {}", result);
  }
}
