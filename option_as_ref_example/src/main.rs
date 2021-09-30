fn main() {
    // println!("Hello, world!");

    // std::option::Option::as_ref()
    // Converts from &Option<T> to Option<&T>.

    let text: Option<String> = Some("Hello, world!".to_string());
    // First, cast `Option<String>` to `Option<&String>` with `as_ref`,
    // then consume *that* with `map`, leaving `text` on the stack.
    let text_length: Option<usize> = text.as_ref().map(|s| s.len());
    println!(
        "still can print text: {:?}, text_length: {:?}",
        text, text_length
    );
}
