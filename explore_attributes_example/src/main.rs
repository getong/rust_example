#[cfg(target_os = "linux")]
fn are_you_on_linux() {
    println!("You're running linux!");
}

#[cfg_attr(feature = "debug-mode", derive(Debug))]
struct Test {
    value: i32,
}

#[inline]
fn add(x: i32, y: i32) -> i32 {
    x + y
}

fn main() {
    // println!("Hello, world!");
    // are_you_on_linux();

    println!("sum : {}", add(2, 3));
}


// copy from https://medium.com/@luishrsoares/exploring-rust-attributes-in-depth-ac172993d568