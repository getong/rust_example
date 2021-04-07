fn main() {
    println!("Hello, world!");
}

#[no_mangle]
pub fn double(n: i32) -> i32 {
    n * 2
}
