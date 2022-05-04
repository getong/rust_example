use std::env;

fn main() {
    // println!("Hello, world!");

    for arg in std::env::args() {
        println!("arg: {}", arg);
    }

    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    let path: &str = &args[0];
    if path.contains("/debug/") {
        println!("The development app is running");
    } else if path.contains("/release/") {
        println!("The production server is running");
    } else {
        panic!("The setting is neither debug or release");
    }
}
