use std::{mem::swap, pin::Pin, task::Poll, time::Duration};

use futures::Future;
use tokio::{macros::support::poll_fn, time::sleep};

#[tokio::main]
async fn main() {
  let mut sleep1 = sleep(Duration::from_secs(1));
  let mut sleep2 = sleep(Duration::from_secs(1));

  {
    // let's use `sleep1` pinned exactly _once_
    let mut sleep1 = unsafe { Pin::new_unchecked(&mut sleep1) };

    // this creates a future whose poll method is the closure argument
    poll_fn(|cx| {
      // we poll `sleep1` once, throwing away the result...
      let _ = sleep1.as_mut().poll(cx);

      // ...and resolve immediately
      Poll::Ready(())
    })
    .await;
  }

  // then, let's use `sleep1` unpinned:
  swap(&mut sleep1, &mut sleep2);
  // by this point, `sleep1` has switched places with `sleep2`

  // finally, let's await both sleep1 and sleep2
  sleep1.await;
  sleep2.await;
}
