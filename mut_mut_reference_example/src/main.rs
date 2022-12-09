fn main() {
    // println!("Hello, world!");
    let mut a: i32 = 10;
    let mut b: &mut i32 = &mut a;
    let c: &mut &mut i32 = &mut b;
    **c += 10;
    println!("a:  {}", a);

    let mut d = &mut 10;
    let e = &mut d;
    **e += 10;
    println!("d:  {}", *d);
}
