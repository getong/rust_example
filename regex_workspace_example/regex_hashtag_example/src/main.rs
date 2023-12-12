use lazy_static::lazy_static;

use regex::Regex;
use std::collections::HashSet;

fn extract_hashtags(text: &str) -> HashSet<&str> {
  lazy_static! {
    static ref HASHTAG_REGEX: Regex = Regex::new(r"\#[a-zA-Z][0-9a-zA-Z_]*").unwrap();
  }
  HASHTAG_REGEX
    .find_iter(text)
    .map(|mat| mat.as_str())
    .collect()
}

fn main() {
  let tweet = "Hey #world, I just got my new #dog, say hello to Till. #dog #forever #2 #_ ";
  let tags = extract_hashtags(tweet);
  assert!(tags.contains("#dog") && tags.contains("#forever") && tags.contains("#world"));
  assert_eq!(tags.len(), 3);
}
