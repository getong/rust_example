use std::thread;
use std::time::Duration;

fn main() {
    // spawn to move variable
    let str = "thread spawn What's that?";
    thread::spawn(move || {
        println!("str = {}", str);
    });

    let str = "What's that?";
    thread::scope(|s| {
        s.spawn(|| {
            println!("str = {}", str);
        });
    });

    let str = "third What's that?";
    thread::scope(|s| {
        s.spawn(|| {
            println!("str = {}", str);
        });
        s.spawn(|| {
            println!("str = {}", str);
        });
        s.spawn(|| {
            println!("str = {}", str);
        });
    });

    let mut i = 0;
    thread::scope(|s| {
        s.spawn(|| {
            thread::sleep(Duration::from_millis(1000));
            i += 1;
            println!("i: {}", i);
        });
    });
    println!("i = {}", i);

    let mut a = vec![1, 2, 3];
    let mut x = 0;

    thread::scope(|s| {
        s.spawn(|| {
            println!("hello from the first scoped thread");
            // We can borrow `a` here.
            dbg!(&a);
        });
        s.spawn(|| {
            println!("hello from the second scoped thread");
            // We can even mutably borrow `x` here,
            // because no other threads are using it.
            x += a[0] + a[2];
        });
        println!("hello from the main thread");
    });

    // After the scope, we can modify and access our variables again:
    a.push(4);
    assert_eq!(x, a.len());
    println!("a: {:?}", a);
}
