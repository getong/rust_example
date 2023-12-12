use std::{
  future::Future,
  mem::forget,
  sync::Arc,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  thread,
  time::Duration,
};

// #[macro_export]
// #[allow(unused_macros)]
// macro_rules! syscall {
//     ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
//         let res = unsafe { libc::$fn($($arg, )*) };
//         if res == -1 {
//             Err(std::io::Error::last_os_error())
//         } else {
//             Ok(res)
//         }
//     }};
// }

pub mod net;

pub struct Runtime;

impl Runtime {
  pub fn run<F: Future>(&self, f: F) {
    // create context
    let data = Arc::new(Resource);

    let waker = RawWaker::new(
      Arc::into_raw(data) as *const (),
      &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
    );
    let waker = unsafe { Waker::from_raw(waker) };
    let mut cx = Context::from_waker(&waker);

    // pin to heap
    let mut f = Box::pin(f);

    // start executor
    loop {
      let res = f.as_mut().poll(&mut cx);
      if let Poll::Ready(_v) = res {
        break;
      }
      println!("top future pending, poll next");
      thread::sleep(Duration::from_secs(1));
    }
  }
}
struct Resource;

fn clone_rw(p: *const ()) -> RawWaker {
  let data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };

  // make sure increment reference count of the underlying source
  // clone increment ref count, into_raw consume the cloned and escape drop
  let p = Arc::into_raw(data.clone());
  // do not decrement ref count
  forget(data);

  // new RawWaker with data pointer to same resource
  RawWaker::new(
    p as *const (),
    // the `RawWakerVTable::new` is a magic `const` function can create a object with 'static lifetime
    &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
  )
}

fn wake_rw(p: *const ()) {
  let _data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };
  // todo wakeup, and clean resource
}

fn wake_by_ref_rw(p: *const ()) {
  let data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };
  // todo wakeup
  forget(data);
}

fn drop_rw(p: *const ()) {
  unsafe { Arc::from_raw(p as *const Resource) };
  // decrement reference count by auto drop
}
