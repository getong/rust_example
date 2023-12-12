use std::cell::RefCell;
use std::thread;

thread_local!(static FOO: RefCell<u32> = RefCell::new(1));

fn main() {
  // println!("Hello, world!");
  FOO.with(|f| {
    assert_eq!(*f.borrow(), 1);
    *f.borrow_mut() = 2;
  });

  // each thread starts out with the initial value of 1
  let t = thread::spawn(move || {
    FOO.with(|f| {
      assert_eq!(*f.borrow(), 1);
      *f.borrow_mut() = 3;
    });

    FOO.with(|f| {
      assert_eq!(*f.borrow(), 3);
    });
  });

  // wait for the thread to complete and bail out on panic
  t.join().unwrap();

  // we retain our original value of 2 despite the child thread
  FOO.with(|f| {
    assert_eq!(*f.borrow(), 2);
  });
}
