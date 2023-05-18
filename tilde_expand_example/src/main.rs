fn main() {
    // println!("Hello, world!");
    let bytes = tilde_expand::tilde_expand(b"~/.zshrc");
    println!(
        "expand file: {:?}",
        String::from_utf8(bytes).expect("Invalid UTF-8 sequence")
    );
}
