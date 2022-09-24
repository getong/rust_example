fn strtok<'a>(s: &'a mut &'a str, delimiter: char) -> &'a str {
    if let Some(i) = s.find(delimiter) {
        let prefix = &s[..i];
        let suffix = &s[(i + delimiter.len_utf8())..];
        *s = suffix;
        prefix
    } else {
        let prefix = *s;
        *s = "";
        prefix
    }
}

fn main() {
    //println!("Hello, world!");

    let x1 = "hello world".to_owned();
    let mut x = x1.as_str();
    let hello = strtok(&mut x, ' ');
    assert_eq!(hello, "hello");

    let str = "Hello World";
    println!(" {}", str.to_uppercase());
    let str = "Hello World";
    println!(" {}", str.to_ascii_uppercase());
    let str = "Hello World";
    println!(" {}", str.to_ascii_lowercase());
    let str = "HELLO WORLD";
    println!(" {}", str.to_lowercase());

    let s1: Box<str> = "Hello there!".into();
    println!("s1: {:?}", s1);
}
