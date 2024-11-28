use std::{
  future::Future,
  sync::Arc,
  task::{Context, Poll},
};

use futures::{
  task::{waker_ref, ArcWake}, // to create `context` from type implements ArcWake trait
  FutureExt,                  // to use `Future::boxed` method
};

/// rustc will do the magic below for `async` function
/// 1. automatically implement `Future`trait
/// 2. generate state machine
/// the `fut_top` state will be `Poll::Ready` when and only when all its dependent futures are
/// completed. `fut_top` is a task from executor's view. Tasks are the top-level futures that have
/// been submitted to an executor.
async fn fut_top() {
  println!("poll top future");
  let fake = FakeFuture;
  // rustc do the magic below for `await`,
  // 1. the generated state machine of `fut_top` will depend on `fake` future state
  // 2. when we poll `fut_top`, it will poll `fake` future
  fake.await;
}

struct FakeFuture;

impl Future for FakeFuture {
  type Output = ();

  fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
    static mut SWITCH: bool = false; // pretend  to be pending at the beginning
    println!("poll fake future");

    // We are only to demo how the returned state impact the state machine.
    // In reality, it is managed by `context`, e.g. cx.waker().wake_by_ref()
    unsafe {
      if SWITCH == false {
        SWITCH = true;
        return Poll::Pending;
      }
    }

    return Poll::Ready(());
  }
}

fn run(f: impl Future<Output = ()> + Send + 'static) {
  // ------------------------------------------------------------------------
  // To drive future to completion, we only need to call `poll`.
  // But, in order to satisfy the `poll` interface, we need to create a `context` object.
  // We are not going to use context object at all, so just create a Dummy Task type,
  // and utilize `futures` helper functions to create a context object from it.
  struct DummyTask;
  impl ArcWake for DummyTask {
    fn wake_by_ref(_arc_self: &Arc<Self>) {
      todo!()
    }
  }
  let task = Arc::new(DummyTask);
  let waker = waker_ref(&task);
  let context = &mut Context::from_waker(&*waker);
  // ------------------------------------------------------------------------

  // drive future to completion
  let mut f = f.boxed();
  while let Poll::Pending = f.as_mut().poll(context) {
    println!("pending...");
  }
}
fn main() {
  let f = fut_top(); // future is lazy which means no execution occurs here
  println!("start to drive future!");
  run(f);
  println!("future completed!");
}
