use std::fmt::Debug;

trait Animal: Debug {
    fn walk(&self) {
        println!("walk");
    }
}

impl dyn Animal {
    fn talk() {
        println!("talk");
    }
}

#[derive(Debug)]
struct Person;

impl Animal for Person {}

fn main() {
    // println!("Hello, world!");
    let p = Person;

    p.walk();
    // p.talk();  // can not run this function like this way
    <dyn Animal>::talk();
}
