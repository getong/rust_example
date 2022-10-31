fn main() {
    // println!("Hello, world!");
    let i = 1;
    let add = |x| x + i;

    println!("add result: {}", add(7));
    println!("add result: {}", add(7));

    let mut s = "hello".to_owned();
    let put_str = |add_str| {
        s.push_str(add_str);
        s
    };
    println!("{:?}", put_str(" world"));
    // println!("{:?}", put_str(" world"));

    let mut s = "hello".to_owned();
    let mut put_str = |add_str| s.push_str(add_str);
    println!("{:?}", put_str(" world"));
    println!("{:?}", put_str(" again"));
    println!("{:?}", s);
}
