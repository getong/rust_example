#![feature(lazy_cell)]

use std::{cell::LazyCell, collections::HashMap};

const MEM: LazyCell<HashMap<usize, usize>> = LazyCell::new(|| {
    let mut m = HashMap::new();
    m.insert(0, 0);
    m.insert(1, 1);
    m.insert(2, 2);
    m.insert(3, 6);
    m.insert(4, 24);
    m
});

fn factorial(i: usize) -> usize {
    MEM[&i]
}

fn main() {
    println!("{}", factorial(0));
    println!("{}", factorial(1));
    println!("{}", factorial(2));
    println!("{}", factorial(3));
    println!("{}", factorial(4));
}

// copy from https://www.reddit.com/r/rust/comments/1416qul/how_to_replace_lazy_static_with_the_new_oncecell/
