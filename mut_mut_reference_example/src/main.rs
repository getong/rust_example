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

    // free the stack
    let mut b: &mut _ = &mut [1, 2, 3, 4, 5];
    b[0] = 4;
    println!("b : {:?}", b);

    // b = &mut [3, 4,5, 6,7];
    let mut binding = [3, 4, 5, 6, 7];
    b = &mut binding;
    b[0] = 10;
    println!("b : {:?}", b);
}
