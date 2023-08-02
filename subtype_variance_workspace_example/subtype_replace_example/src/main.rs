// copy from https://www.whexy.com/posts/Variance-in-Rust
fn main() {
    let s = String::new();
    let x = "static str"; // `x` is `&'static str`
    let mut y = &*s; // `y` is `&'s str`
    y = x;
    // Still compilable!
    // Rust automatically shrink the lifetime static to s.

    /// define a function which takes an lifetime sticker `a`
    fn foo<'a>(_: &'a str) {}

    // and you can call the function with
    foo(&s);
    // or
    foo(x);

    let x: &dyn for<'a> Fn(&'a str) -> () = todo!();
    foo2(&x); // that makes sense.

    let y: &dyn Fn(&'static str) -> () = todo!();
    foo2(&y); // should that make sense ???

    let mut x = "Hello"; // x : &'static str
    let z = String::new();
    foo3(&mut x, &z); // foo(&'static str, &'z str)
                      // foo3(x, &z); // foo(&'static str, &'z str)
    drop(z);
    // println!("{}", x); // OOPS!
}

/// define a function which takes a function,
/// which takes a lifetime sticker `a`.
fn foo2<'a>(bar: &dyn Fn(&'a str) -> ()) {
    bar("hello");
}

fn foo3<'a>(s: &mut &'a str, x: &'a str) {
    *s = x;
}
