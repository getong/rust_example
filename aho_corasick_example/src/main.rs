use aho_corasick::{AhoCorasick, PatternID};

fn main() {
  let patterns = &["apple", "maple", "Snapple"];
  let haystack = "Nobody likes maple in their apple flavored Snapple.";

  let ac = AhoCorasick::new(patterns).unwrap();
  let mut matches = vec![];
  for mat in ac.find_iter(haystack) {
    matches.push((mat.pattern(), mat.start(), mat.end()));
  }
  assert_eq!(
    matches,
    vec![
      (PatternID::must(1), 13, 18),
      (PatternID::must(0), 28, 33),
      (PatternID::must(2), 43, 50),
    ]
  );
}
