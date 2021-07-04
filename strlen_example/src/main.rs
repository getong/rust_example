unsafe fn strlen(s: *const u8) -> usize {
    let mut p = s;
    while *p != b'\0' {
        p = p.add(1);
    }
    (p as usize) - (s as usize)
}

fn main() {
    // println!("Hello, world!");
    let word_string: &'static str = "hello world";
    let len = word_string.len();
    println!("len is {} ", len);

    unsafe {
        let s = b"hello\0".as_ptr();
        println!("{:?}", strlen(s));
    }
}
