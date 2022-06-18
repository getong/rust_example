use std::mem::size_of;

fn main() {
    assert_eq!(size_of::<Box<str>>(), 16); // 16
    assert_eq!(size_of::<String>(), 24); // 24
}
