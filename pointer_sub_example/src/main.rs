fn main() {
    // println!("Hello, world!");
    let s: &str = "123";

    unsafe {
        let end: *const u8 = s.as_ptr().add(3);
        println!("{}", *end.sub(1) as char);
        println!("{}", *end.sub(2) as char);
    }
}
