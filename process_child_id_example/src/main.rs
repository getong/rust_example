use std::process::Command;

fn main() {
    // println!("Hello, world!");

    let mut command = Command::new("ls");
    if let Ok(mut child) = command.spawn() {
        println!("Child's ID is {}", child.id());

        child.wait().expect("command wasn't running");
        println!("Child has finished its execution!");
    } else {
        println!("ls command didn't start");
    }
}
