pub fn add_version(s: &str) -> String {
    s.to_string() + " 2018."
}

fn main() {
    println!("Hello, world!");
}

#[test]
fn test_add_version() {
    assert_eq!(add_version("abcd"), String::from("abcd 2018."));
}
