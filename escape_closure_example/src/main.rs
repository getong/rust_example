fn main() {
    // a is Copy
    let a = 1;
    // b is not Copy
    let b = "hello".to_owned();
    let c: Box<dyn Fn() + 'static> = Box::new(move || {
        println!("a: {}", a); // error, borrowed value does not live long enough
        println!("b :{}", b);
    });
    println!("a in the main :{}", a);

    // can not use b here
    // println!("b in the main :{}", b);

    let d = c;
    println!("d in the main :{:?}", d());
}
