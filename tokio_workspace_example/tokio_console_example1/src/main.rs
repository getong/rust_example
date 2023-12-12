use std::thread;
use std::time::Duration;
use tokio::task::Builder;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
  console_subscriber::init(); // for tokio-console

  let cpus = num_cpus::get();
  println!("logical cores: {}", cpus);

  let mut handles = Vec::new();

  let task_a = Builder::new().name("Task A").spawn(async {
    loop {
      println!("   =Task A sleeping... {:?}", thread::current().id());
      sleep(Duration::from_secs(1)).await;
      println!("   =Task A woke up.");
    }
  });
  handles.push(task_a);

  for i in 0..cpus + 1 {
    let task_b = Builder::new()
      .name(&format!("Task B-{}", i))
      .spawn(async move {
        loop {
          println!("#Task B-{} sleeping... {:?}", i, thread::current().id());
          thread::sleep(Duration::from_secs(5));
          println!("#Task B-{} woke up.", i);
        }
      });
    handles.push(task_b)
  }
}
