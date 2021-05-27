use std::fmt::Debug;

trait TraitName: Debug {}

impl TraitName for i32 {}

fn dyn_demo() -> Box<dyn TraitName> {
    Box::new(5)
}

fn impl_demo() -> impl TraitName {
    5
}

fn dyn_closure_demo() -> Box<dyn Fn(i32) -> i32> {
    Box::new(|x| x + 1)
}

fn impl_closure_demo() -> impl Fn(i32) -> i32 {
    |x| x + 1
}

fn main() {
    //println!("Hello, world!");
    let a = dyn_demo();
    println!("a : {:?}", a);

    let a1 = impl_demo();
    println!("a1 : {:?}", a1);

    let b_closure = dyn_closure_demo();
    let b1_closure = impl_closure_demo();
    let b_value = b_closure(1);
    let b1_value = b1_closure(1);
    println!("b : {:?}", b_value);
    println!("b1 : {:?}", b1_value);
}
