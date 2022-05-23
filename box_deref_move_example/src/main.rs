fn main() {
    let s = Box::new("hello".to_string());
    // the pointer on the stack
    println!("{:p}", &s);
    // the pointer on the heap
    println!("{:p}", s.as_ptr());
    // DerefMove
    let s2 = *s;
    // can not use `s` anymore
    // println!("{:p}", s.as_ptr()); // Moved s
    println!("{:p}", s2.as_ptr());
}
