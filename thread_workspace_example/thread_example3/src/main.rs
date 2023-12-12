use std::thread;

fn main() {
  println!("So we start the program here!");
  let t1 = thread::spawn(move || {
    thread::sleep(std::time::Duration::from_millis(200));
    println!("We create tasks which gets run when they're finished!");
  });

  let t2 = thread::spawn(move || {
    thread::sleep(std::time::Duration::from_millis(100));
    println!("We can even chain callbacks...");
    let t3 = thread::spawn(move || {
      thread::sleep(std::time::Duration::from_millis(50));
      println!("...like this!");
    });
    t3.join().unwrap();
  });
  println!("While our tasks are executing we can do other stuff here.");

  t1.join().unwrap();
  t2.join().unwrap();
}
