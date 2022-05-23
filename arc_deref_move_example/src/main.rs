use std::sync::Arc;

fn main() {
    let s = Arc::new("hello".to_string());
    println!("{:p}", &s);
    println!("{:p}", s.as_ptr());
    // DerefMove Error : cannot move out of an `Arc`

    // let s2 = *s;
    // println!("{:p}", s.as_ptr()); // Moved s

    let s2 = &*s;
    println!("{:p}", s2.as_ptr());
}
