use indexmap::IndexSet;

fn main() {
  // println!("Hello, world!");
  // Collects which letters appear in a sentence.
  let letters: IndexSet<_> = "a short treatise on fungi".chars().collect();

  assert!(letters.contains(&'s'));
  assert!(letters.contains(&'t'));
  assert!(letters.contains(&'u'));
  assert!(!letters.contains(&'y'));
}
