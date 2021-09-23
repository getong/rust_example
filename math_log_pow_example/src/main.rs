#![feature(int_log)]

fn main() {
    // println!("Hello, world!");

    assert_eq!(5i8.log(5), 1);

    // i32::MAX
    assert_eq!(i32::MAX, max_i32_value());

    let max_3_pow_value: i32 = max_3_pow_value();
    println!("max_3_pow_value: {}", max_3_pow_value);
}

fn max_i32_value() -> i32 {
    // i32::MAX is the max i32 value
    // `2_i32.pow(31) - 1`,  the code can not be running
    2_i32.pow(30) + (2_i32.pow(30) - 1)
}

// the max number 3 value
fn max_3_pow_value() -> i32 {
    3_i32.pow(i32::MAX.log(3))
}
