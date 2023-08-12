use bytes::{Buf, BufMut, BytesMut};
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
    remaining_mut_example();
}

fn has_remaining_example() {
    let mut buf = Cursor::new(b"a");

    assert!(buf.has_remaining());

    buf.get_u8();

    assert!(!buf.has_remaining());

    // bytes equal &str
    assert_eq!(b"abc", &[97, 98, 99]);
}

fn remaining_mut_example() {
    let mut dst = [0; 10];
    let mut buf = &mut dst[..];

    let original_remaining = buf.remaining_mut();
    buf.put(&b"hello"[..]);

    assert_eq!(original_remaining - 5, buf.remaining_mut());
}
