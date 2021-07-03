extern crate itertools;
use itertools::Itertools;

fn main() {
    // println!("Hello, world!");
    let arr = [
        10u32, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    for tuple in arr.iter().batching(|it| match it.next() {
        None => None,
        Some(x) => match it.next() {
            None => None,
            Some(z) => match it.next() {
                None => None,
                Some(y) => Some((x, y, z)),
            },
        },
    }) {
        println!("tuple: {:?}", tuple);
    }

    println!();

    let arr = [
        10u32, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    println!("arr: {:?}", arr);
    for tuple in arr.iter().tuples::<(_, _, _)>() {
        println!("three elements tuple {:?}", tuple);
    }
}
