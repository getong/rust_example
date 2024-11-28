use core::{mem::MaybeUninit, ptr};
use std::sync::{mpsc, Once};

static mut CHANNEL: MaybeUninit<(mpsc::Sender<usize>, mpsc::Receiver<usize>)> =
  MaybeUninit::uninit();
static CHANNEL_INIT: Once = Once::new();

#[inline]
fn get_channel() -> &'static (mpsc::Sender<usize>, mpsc::Receiver<usize>) {
  CHANNEL_INIT.call_once(|| unsafe {
    ptr::write(CHANNEL.as_mut_ptr(), mpsc::channel());
  });

  unsafe { &*CHANNEL.as_ptr() }
}

fn main() {
  get_channel(); // safe because call_once will sync
}
