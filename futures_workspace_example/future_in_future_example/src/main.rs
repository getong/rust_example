use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use futures::channel::oneshot; // futures-preview@0.3.0-alpha.17

pub struct ShutdownHandle {
  sender: oneshot::Sender<()>,
  receiver: oneshot::Receiver<()>,
}

impl ShutdownHandle {
  pub fn new() -> Self {
    let (sender, receiver) = oneshot::channel();
    Self { sender, receiver }
  }

  pub fn shutdown(self) -> Result<(), ()> {
    self.sender.send(())
  }
}

// impl Future for ShutdownHandle {
//   type Output = ();

//   fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
//     self.receiver.poll(&mut cx).map(|_| ())
//   }
// }

impl Future for ShutdownHandle {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    // I copied this code from Stack Overflow without reading the text that
    // told me how to verify that this code uses `unsafe` correctly.
    unsafe { self.map_unchecked_mut(|s| &mut s.receiver) }
      .poll(cx)
      .map(|_| ())
  }
}

#[tokio::main]
async fn main() {
  let runner = ShutdownHandle::new();
  assert!(runner.shutdown().is_ok());
}

// copy from https://stackoverflow.com/questions/57369123/no-method-named-poll-found-for-a-type-that-implements-future
