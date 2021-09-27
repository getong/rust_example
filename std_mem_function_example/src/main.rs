use std::mem;

fn main() {
    // println!("Hello, world!");
    let mut v: Vec<String> = Vec::new();
    for i in 101..106 {
        v.push(i.to_string());
    }

    let fifth = v.pop().expect("vector empty!");
    assert_eq!(fifth, "105");

    let second = v.swap_remove(1);
    assert_eq!(second, "102");

    let third = mem::replace(&mut v[2], "substitute".to_string());
    assert_eq!(third, "103");

    assert_eq!(v, vec!["101", "104", "substitute"]);

    swap_function();
}

fn swap_function() {
    let mut x = 5;
    let mut y = 42;

    mem::swap(&mut x, &mut y);

    assert_eq!(42, x);
    assert_eq!(5, y);
}
