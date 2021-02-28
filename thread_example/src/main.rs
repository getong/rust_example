use std::thread;

fn main() {
    // println!("Hello, world!");

    let child = thread::spawn(|| {
        println!("Thread!");
        String::from("Much concurrent, such wow!")
    });
    print!("Hello ");
    let value = child.join().expect("Failed joining child thread");
    println!("{}", value);
}
