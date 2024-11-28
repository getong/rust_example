use std::{thread, time::Instant};

fn main() {
  // println!("Hello, world!");
  let start_time = Instant::now();
  let child = thread::spawn(|| {
    println!("Thread!");
    String::from("Much concurrent, such wow!")
  });

  let thread: &thread::Thread = child.thread();
  println!("thread id: {:?}", thread.id());

  // print!("Hello ");
  let value = child.join().expect("Failed joining child thread");

  let d = Instant::now().duration_since(start_time);
  let delta = d.as_millis();

  println!("value: {}, processed time:{}", value, delta);

  // thread::spawn(f);
  // thread::spawn(f);
  // println!("Hello from the main thread.");
  let t1 = thread::spawn(f);
  let t2 = thread::spawn(f);
  println!("Hello from the main thread.");
  t1.join().unwrap();
  t2.join().unwrap();
}

fn f() {
  println!("Hello from another thread!");
  let id = thread::current().id();
  println!("This is my thread id: {id:?}");
}
