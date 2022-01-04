// Both of these files are read at *compile time*
const FILE_CONTENT: &str = include_str!("./Cargo.toml");
const BINARY_FILE_CONTENT: &[u8] = include_bytes!("./Cargo.toml");

fn main() {
    println!("{}", FILE_CONTENT); // Output: file content as string
}
