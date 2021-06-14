// use trait method, the trait itself muse be in scope.
// Otherwise, all its methods are hidden
use std::io::Write;

fn main() {
    // println!("Hello, world!");
    let mut buf: Vec<u8> = vec![];
    let _ = buf.write_all(b"hello");
    println!("buf: {:?}", buf);
}
