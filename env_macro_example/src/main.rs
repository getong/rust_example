const PATH: &str = env!("PATH");

fn main() {
    // println!("Hello, world!");
    let path: &'static str = env!("PATH");
    println!(
        "the static $PATH variable at the time of compiling was: {}",
        path
    );

    println!();

    println!(
        "the const $PATH variable at the time of compiling was: {}",
        &*PATH
    );
}
