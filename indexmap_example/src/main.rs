use indexmap::IndexMap;

fn main() {
  // println!("Hello, world!");
  // count the frequency of each letter in a sentence.
  let mut letters = IndexMap::new();
  for ch in "a short treatise on fungi".chars() {
    *letters.entry(ch).or_insert(0) += 1;
  }

  assert_eq!(letters[&'s'], 2);
  assert_eq!(letters[&'t'], 3);
  assert_eq!(letters[&'u'], 1);
  assert_eq!(letters.get(&'y'), None);
}
