unsafe fn strlen(s: *const u8) -> usize {
    let mut p = s;
    while *p != b'\0' {
        p = p.add(1);
    }
    (p as usize) - (s as usize)
}

fn main() {
    // println!("Hello, world!");
    unsafe {
        let s = b"hello\0".as_ptr();
        println!("{:?}", strlen(s));
    }
}
