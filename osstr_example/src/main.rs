use std::ffi::OsStr;
use std::path::Path;

fn main() {
    // println!("Hello, world!");
    let path = Path::new("C:/Users/JimB/Downloads/Fedora.iso");
    let mut iterator = path.iter();
    assert_eq!(iterator.next(), Some(OsStr::new("C:")));
    assert_eq!(iterator.next(), Some(OsStr::new("Users")));
    assert_eq!(iterator.next(), Some(OsStr::new("JimB")));
}
