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

    // len() and .chars().count()
    println!("{}", "a".len()); // .len() gives the size in bytes
    println!("{}", "ß".len());
    println!("{}", "国".len());
    println!("{}", "𓅱".len());

    let slice = "Hello!";
    println!(
        "Slice is {} bytes and also {} characters.",
        slice.len(),
        slice.chars().count()
    );
    let slice2 = "안녕!";
    println!(
        "Slice2 is {} bytes but only {} characters.",
        slice2.len(),
        slice2.chars().count()
    );
}

fn add_version(s: &mut String) -> String {
    s.push_str(" 2019!!");
    s.to_string()
}

fn add_lang(s: &mut String) -> String {
    s.push_str(" lang.");
    s.to_string()
}
