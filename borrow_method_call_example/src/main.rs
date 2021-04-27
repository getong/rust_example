fn main() {
    let s = String::from("hello");
    let len1 = String::len(&s);
    let len2 = s.len(); // shorthand for the above
    println!("len1 = {} = len2 = {}", len1, len2);

    let mut s1 = String::from("Hello");
    let s2 = String::from(", world");
    String::push_str(&mut s1, &s2);
    s1.push_str(&s2); // shorthand for the above
    println!("{}", s1); // prints "Hello, world, world"

    let mut x = String::from("Hello");
    let y = &mut x;
    world(y);
    let z = &mut x; // OK, because y's lifetime has ended (last use was on previous line)
    world(z);
    x.push_str("!!"); // Also OK, because y and z's lifetimes have ended
    println!("{}", x);

    let r = Rect { w: 30, h: 50 };

    println!("The area of the rectangle is {} square pixels.", area(&r));
}

fn world(s: &mut String) {
    s.push_str(", world");
}

struct Rect {
    w: u32,
    h: u32,
}
