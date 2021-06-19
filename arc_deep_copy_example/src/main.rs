// use std::rc::Rc;
use std::sync::Arc;

fn main() {
    let arc = Arc::new("Hello".to_string());
    let mut cloned = (*arc).clone();
    cloned.truncate(4);

    // verify that it's modified
    println!("{:?}", cloned); // "Hell"
                              // verify that the original was not
    println!("{:?}", arc); // "Hello"
    many_arc_reference();
}

fn many_arc_reference() {
    let arc3 = Arc::new(Arc::new(Arc::new("Hello".to_string())));
    let mut cloned = String::clone(&arc3);
    cloned.truncate(4);

    // verify that it's modified
    println!("{:?}", cloned); // "Hell"
                              // verify that the original was not
    println!("{:?}", arc3); // "Hello"
}
