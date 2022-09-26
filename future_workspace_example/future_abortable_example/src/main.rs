use futures::executor::block_on;
use futures::future::{ready, AbortHandle, Abortable, Aborted};
// use futures::prelude::*;

#[tokio::main]
async fn main() {
    // println!("Hello, world!");

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let future = Abortable::new(ready(2), abort_registration);
    abort_handle.abort();
    assert_eq!(block_on(future), Err(Aborted));
}
