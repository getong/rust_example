fn main() {
  println!("Size of a char: {}", std::mem::size_of::<char>()); // 4 bytes
  println!("Size of string containing 'a': {}", "a".len()); // .len() gives the size of the string in bytes
  println!("Size of string containing 'ß': {}", "ß".len());
  println!("Size of string containing '国': {}", "国".len());
  println!("Size of string containing '𓅱': {}", "𓅱".len());

  let mut upper = 's'.to_uppercase();
  assert_eq!(upper.next(), Some('S'));
  assert_eq!(upper.next(), None);

  // The uppercase form of the German letter "sharp S" is "SS":
  let mut upper = 'ß'.to_uppercase();
  assert_eq!(upper.next(), Some('S'));
  assert_eq!(upper.next(), Some('S'));
  assert_eq!(upper.next(), None);

  // Unicode says to lowercase Turkish dotted capital 'İ' to 'i'
  // followed by `'\u{307}'`, COMBINING DOT ABOVE, so that a
  // subsequent conversion back to uppercase preserves the dot.
  let ch = 'İ'; // `'\u{130}'`
  let mut lower = ch.to_lowercase();
  assert_eq!(lower.next(), Some('i'));
  assert_eq!(lower.next(), Some('\u{307}'));
  assert_eq!(lower.next(), None);

  let emoji = "\u{1f600}";
  println!("emoji: {} \n", emoji);
}
