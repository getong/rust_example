fn main() {
    let a = 1;
    let c: Box<dyn Fn() + 'static> = Box::new(move || {
        println!("a: {}", a); // error, borrowed value does not live long enough
    });
    println!("a in the main :{}", a);
    let d = c;
    println!("d in the main :{:?}", d());
}
