struct Foo {
    a: i32,
    b: i32,
    c: i32,
}

fn main() {
    // println!("Hello, world!");
    let mut x = Foo { a: 0, b: 0, c: 0 };
    let a = &mut x.a;
    let b = &mut x.b;
    let c = &x.c;
    *b += 1;
    let c2 = &x.c;
    *a += 10;
    println!("{} {} {} {}", a, b, c, c2);

    // let mut x = [1, 2, 3];
    // let a = &mut x[0];
    // let b = &mut x[1];
    // println!("{} {}", a, b);

    let mut x = [1, 2, 3];
    println!("before x : {:?}", x);
    let (left, _right) = x.split_at_mut(1);
    left[0] =3;
    println!("after x : {:?}", x);
}

// copy from https://doc.rust-lang.org/nomicon/borrow-splitting.html