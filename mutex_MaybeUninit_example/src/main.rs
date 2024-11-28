use std::{mem::MaybeUninit, sync::Mutex, thread};

static mut M: MaybeUninit<Mutex<u32>> = MaybeUninit::uninit();

// deadlock example
// fn main() {
//    let a = Mutex::new(0);
//    let b = a.lock().unwrap();
//    let _c = a.lock().unwrap();
//    println!("{}", *b);
//}

fn main() {
  thread::spawn(move || unsafe {
    let m = M.as_mut_ptr();
    m.write(Mutex::new(3));
  })
  .join()
  .expect("thread::spawn failed");

  unsafe {
    let m = M.as_mut_ptr();
    m.write(Mutex::new(2));
  }

  unsafe {
    assert_eq!(*M.as_ptr().read().lock().unwrap(), 2);
  }
}
