use kmp_example3::KMP;

fn main() {
  let pattern = "abcabca";
  let kmp = KMP::new(pattern);
  debug_assert_eq!(3, kmp.index_of_any("abxabcabcaby"));
  debug_assert_eq!(-1, kmp.index_of_any("abxabdabcaby"));
  debug_assert_eq!(0, kmp.index_of_any("abcabcax"));
  debug_assert_eq!(1, kmp.index_of_any("aabcabcax"));

  let pattern = "aaaaacdaac";
  let kmp = KMP::new(pattern);
  println!("kmp : {:?}", kmp);
}
