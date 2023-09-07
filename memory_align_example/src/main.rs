use std::mem;
use std::rc::Rc;

// 12 bytes
#[repr(C)]
struct Member {
    // 1 byte
    active: bool,
    // 4 bytes
    age: i32,
    // 1 byte
    admin: bool,
}

// 8 bytes
struct Member2 {
    // 1 byte
    active: bool,
    // 4 bytes
    age: i32,
    // 1 byte
    admin: bool,
}

fn main() {
    println!("Member: {} bytes", mem::size_of::<Member>());
    println!("Member2: {} bytes", mem::size_of::<Member2>());

    assert_eq!(mem::size_of::<&[u64]>(), 16);

    assert_eq!(mem::size_of::<Vec<u64>>(), 24);
    assert_eq!(mem::size_of::<&Vec<u64>>(), 8);
    assert_eq!(mem::size_of::<Rc<u64>>(), 8);
    assert_eq!(mem::size_of::<&Rc<u64>>(), 8);
}
