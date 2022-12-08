fn main() {
    // println!("Hello, world!");
    let mut a: i32 = 10;
    let mut b: &mut i32 = &mut a;
    let ccd : &mut &mut i32 = &mut b;
    **c += 10;
    println!("a:  {}", a);
}
