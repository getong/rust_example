fn main() {
    // println!("Hello, world!");
    let a: String = "Hello".to_string();
    let b = String::from("Hello");
    let c = "World".to_owned();
    let d = c.clone();
    println!("a: {}, b: {}, c: {}, d: {}", a, b, c, d);

    let mut empty_string = String::new();
    let empty_string_with_capacity = String::with_capacity(50);
    let string_from_bytestring: String =
        String::from_utf8(vec![82, 85, 83, 84]).expect("Creating String from bytestring failed");
    println!("Length of the empty string is {}", empty_string.len());
    println!(
        "Length of the empty string with capacity is {}",
        empty_string_with_capacity.len()
    );
    println!(
        "Length of the string from a bytestring is {}",
        string_from_bytestring.len()
    );
    println!("Bytestring says {}", string_from_bytestring);
    empty_string.push('1');
    println!("1) Empty string now contains {}", empty_string);
    empty_string.push_str("2345");
    println!("2) Empty string now contains {}", empty_string);

    println!(
        "Length of the previously empty string is now {}",
        empty_string.len()
    );

    let mut lang = String::from("rust");
    let rust1 = add_version(&mut lang);
    println!("{:?}", rust1);
    let rust2 = add_lang(&mut lang);
    println!("{:?}", rust2);
}

fn add_version(s: &mut String) -> String {
    s.push_str(" 2019!!");
    s.to_string()
}
fn add_lang(s: &mut String) -> String {
    s.push_str(" lang.");
    s.to_string()
}
