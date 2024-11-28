// use std::time::Instant;
use crate::message::Message;

pub struct ReceiveMessage<T> {
  pub data: Message<T>,
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn is_message() {
    let message = 12;
    let time = Instant::now();
    let m = Message::new(message, time);

    let recv_data = ReceiveMessage { data: m };

    assert_eq!(*recv_data.data.message.as_ref(), 12);
  }
}
