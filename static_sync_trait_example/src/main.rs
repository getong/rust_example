use std::sync::Mutex;
use std::thread;

static I: i32 = 4;
static M: Mutex<i32> = Mutex::new(5);

// use std::cell::RefCell;
// static C: RefCell<i32> = RefCell::new(5);

fn main() {
  let mut handles = vec![];
  for _ in 0..10 {
    // let data = data.clone();
    let handle = thread::spawn(|| {
      println!("The data is: {}", &I);
      *M.lock().unwrap() += 1;
      println!("the mutex is {:?}", M.lock());
      // println!("the refcell: {:?}", C);
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }

  println!("The data is: {}", I);
  println!("the final mutex is {:?}", M.lock());
}
