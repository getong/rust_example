use std::thread;
fn main() {
    let mut health = 12;

    thread::spawn(move || {
        health *= 2;
        println!("thread health:{} ", health);
    });

    println!("{}", health);
}
