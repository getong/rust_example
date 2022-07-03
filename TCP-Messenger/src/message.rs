use std::time::Instant;


pub struct Message<T> {
    pub message: Box<T>,
    pub time: Instant
}

impl<T> Message<T> {
    pub fn new(message: T, time: Instant) -> Self {
        Message {
            message: Box::new(message),
            time: time
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn add_message() {
        let time = Instant::now();
        let msg = Message::new("test", time);

        assert_eq!(*msg.message.as_ref(), "test");
    }
}