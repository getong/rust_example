use std::process;

fn get_process_id() -> u32 {
    process::id()
}

fn main() {
    println!("Hello, world!");
    println!("the process id is {}", get_process_id());
}
