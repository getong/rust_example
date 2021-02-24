use std::cell::RefCell;

#[derive(Debug)]
struct Point {
    x: RefCell<i32>,
    y: i32,
}

fn main() {
    let x = RefCell::new(1);
    let y = &x;
    let z = &x;
    *x.borrow_mut() = 2;
    *y.borrow_mut() = 3;
    *z.borrow_mut() = 4;
    println!("{:?}", x);

    let p = Point {
        x: RefCell::new(1),
        y: 2,
    };
    let p1 = &p;
    let p2 = &p;
    *p1.x.borrow_mut() = 3;
    *p2.x.borrow_mut() = 4;

    println!("{:?}", p);
}
