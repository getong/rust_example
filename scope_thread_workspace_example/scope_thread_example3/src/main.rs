/*
pub struct Scope<'scope, 'env: 'scope> {
  data: Arc<ScopeData>,
  scope: PhantomData<&'scope mut &'scope ()>,
  env: PhantomData<&'env mut &'env ()>,
}

pub(super) struct ScopeData {
  num_running_threads: AtomicUsize,
  a_thread_panicked: AtomicBool,
  main_thread: Thread,
}

let scope = Scope {
  data: Arc::new(ScopeData {
    num_running_threads: AtomicUsize::new(0),
    main_thread: current(),
    a_thread_panicked: AtomicBool::new(false),
  }),
  env: PhantomData,
  scope: PhantomData,
};

while scope.data.num_running_threads.load(Ordering::Acquire) != 0 {
  park();
}


// Book-keeping so the scope knows when it's done.
if let Some(scope) = &self.scope {
  scope.decrement_num_running_threads(unhandled_panic);
}

copy from https://medium.com/@KevinBGreene/async-programming-in-rust-part-2-diving-into-scoped-threads-50aace437756

*/
use std::thread;
use std::time::Duration;

fn main() {
  let mut i = 0;
  thread::scope(|s| {
    s.spawn(|| {
      thread::sleep(Duration::from_millis(1000));
      i += 1;
    });
  });
  println!("i = {}", i);
}
