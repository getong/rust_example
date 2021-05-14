fn extension(file_name: &str) -> Option<&str> {
    find(file_name, '.').map(|i| &file_name[i + 1..])
}

fn find(haystack: &str, needle: char) -> Option<usize> {
    for (offset, c) in haystack.char_indices() {
        if c == needle {
            return Some(offset);
        }
    }
    None
}

fn main() {
    assert_eq!(extension("foo.rs").unwrap_or("rs"), "rs");
    assert_eq!(extension("foo").unwrap_or("rs"), "rs");
}
