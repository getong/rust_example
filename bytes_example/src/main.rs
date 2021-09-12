use bytes::Buf;
use bytes::{BufMut, BytesMut};
use std::io::Cursor;

fn main() {
    // println!("Hello, world!");
    let mut buf = BytesMut::with_capacity(64);

    buf.put_u8(b'h');
    buf.put_u8(b'e');
    buf.put(&b"llo"[..]);

    assert_eq!(&buf[..], b"hello");

    // Freeze the buffer so that it can be shared
    let a = buf.freeze();

    // This does not allocate, instead `b` points to the same memory.
    let b = a.clone();

    assert_eq!(&a[..], b"hello");
    assert_eq!(&b[..], b"hello");

    has_remaining_example();
}

fn has_remaining_example() {
    let mut buf = Cursor::new(b"a");

    assert!(buf.has_remaining());

    buf.get_u8();

    assert!(!buf.has_remaining());
}
