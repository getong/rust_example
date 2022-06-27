fn main() {
    // println!("Hello, world!");
    let mut num = 32_u32;

    let a = &mut num;
    let b: &mut _ = a;
    *b += 1;
    *a += 1;
    println!("a:{}", a);
}
