use std::sync::{mpsc, Mutex, MutexGuard, Once};

type Channel = (mpsc::Sender<usize>, mpsc::Receiver<usize>);

static CHANNEL: Mutex<Option<Channel>> = Mutex::new(None);
static CHANNEL_INIT: Once = Once::new();

#[inline]
fn get_channel() -> MutexGuard<'static, Option<Channel>> {
  CHANNEL_INIT.call_once(|| {
    *CHANNEL.lock().unwrap() = Some(mpsc::channel());
  });

  CHANNEL.lock().unwrap()
}

fn main() {
  {
    let channel = get_channel();
    let (sender, _) = channel.as_ref().unwrap();
    sender.send(42).unwrap();
  }

  let channel = get_channel();
  let (_, receiver) = channel.as_ref().unwrap();
  assert_eq!(receiver.recv().unwrap(), 42);
}
