use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

fn main() {
  let mut buffer = AllocRingBuffer::with_capacity(2);

  // First entry of the buffer is now 5.
  buffer.push(5);

  // The last item we pushed is 5
  assert_eq!(buffer.get(-1), Some(&5));

  // Second entry is now 42.
  buffer.push(42);
  assert_eq!(buffer.peek(), Some(&5));
  assert!(buffer.is_full());

  // Because capacity is reached the next push will be the first item of the buffer.
  buffer.push(1);
  assert_eq!(buffer.to_vec(), vec![42, 1]);
}
