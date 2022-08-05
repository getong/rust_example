use std::mem;

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
}
