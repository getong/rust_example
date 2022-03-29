// Both of these files are read at *compile time*
const FILE_CONTENT: &str = include_str!("../Cargo.toml");
const BINARY_FILE_CONTENT: &[u8] = include_bytes!("../Cargo.toml");

fn main() {
    println!("file:{}\n\n", FILE_CONTENT); // Output: file content as string
    println!("binary:{:?}\n\n", BINARY_FILE_CONTENT); // Output: file content as string

    println!("the main is :\n\n{}", include_str!("./main.rs"));
}
