use regex::Regex;
use regex::RegexSet;

fn simple_match_example() {
  let re = Regex::new(r"rust").unwrap();
  assert!(re.is_match("I love rust!"));
}

fn case_insensitive_match_example() {
  let re = Regex::new(r"(?i)rust").unwrap();
  assert!(re.is_match("I love Rust!"));
}

fn capturing_groups_example() {
  let re = Regex::new(r"(\w+) (\w+)").unwrap();
  let caps = re.captures("hello world").unwrap();

  assert_eq!("hello", &caps[1]);
  assert_eq!("world", &caps[2]);
}

fn named_capturing_groups_example() {
  let re = Regex::new(r"(?P<word>\w+) world").unwrap();
  let caps = re.captures("hello world").unwrap();

  assert_eq!("hello", &caps["word"]);
}

fn replacing_matches_example() {
  let re = Regex::new(r"\brust\b").unwrap();
  let result = re.replace_all("I love rust!", "Rust");
  assert_eq!(result, "I love Rust!");
}

fn replacing_with_a_function_example() {
  let re = Regex::new(r"\d+").unwrap();
  let result = re.replace_all("123 456 789", |caps: &regex::Captures| {
    let num: i32 = caps[0].parse().unwrap();
    (num * 2).to_string()
  });
  assert_eq!(result, "246 912 1578");
}

fn iterating_over_matches_example() {
  let re = Regex::new(r"\b\w+\b").unwrap();
  for cap in re.captures_iter("hello world") {
    println!("{}", cap.get(0).unwrap().as_str());
  }
}

fn splitting_a_string_example() {
  let re = Regex::new(r"\s+").unwrap();
  let parts: Vec<_> = re.split("split on     whitespace").collect();
  assert_eq!(parts, vec!["split", "on", "whitespace"]);
}

fn regex_set_example() {
  let set = RegexSet::new(&[r"rust", r"java", r"python"]).unwrap();
  let matches = set.matches("I love rust and python!");
  assert!(matches.matched(0));
  assert!(!matches.matched(1));
  assert!(matches.matched(2));
}

// fn advanced_lookaround_example() {
//     let re = Regex::new(r"(?<=@)\w+").unwrap();
//     let caps = re.captures("Follow us @mymail!");
//     assert_eq!("mymail", &caps.unwrap()[0]);
// }

fn main() {
  // println!("Hello, world!");
  simple_match_example();
  case_insensitive_match_example();
  capturing_groups_example();
  named_capturing_groups_example();
  replacing_matches_example();
  replacing_with_a_function_example();
  iterating_over_matches_example();
  splitting_a_string_example();
  regex_set_example();
  // advanced_lookaround_example();
}

// copy from https://medium.com/@TechSavvyScribe/rust-regex-10-practical-examples-ec11527b8b84
