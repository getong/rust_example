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

  // unwrap_or_else
  let k = 10;
  assert_eq!(Some(4).unwrap_or_else(|| 2 * k), 4);
  assert_eq!(None.unwrap_or_else(|| 2 * k), 20);

  // unwrap_or_default
  let x: Option<u32> = None;
  let y: Option<u32> = Some(12);

  assert_eq!(x.unwrap_or_default(), 0);
  assert_eq!(y.unwrap_or_default(), 12);
}
