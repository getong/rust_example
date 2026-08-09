// 使用 crates.io 上的 `kmp` 库（https://crates.io/crates/kmp）进行字符串匹配，
// 不再手写 KMP 算法。该库基于 Knuth–Morris–Pratt 算法，输入为任意 &[T] 切片。
use kmp::{kmp_find, kmp_find_with_lsp_table, kmp_match, kmp_table};

fn main() {
  let haystack = "BBC ABCDAB ABCDABCDABDE";
  let needle = "ABCDABD";

  // kmp_find: 返回第一次匹配的起始下标
  match kmp_find(needle.as_bytes(), haystack.as_bytes()) {
    Some(pos) => println!("`{needle}` 在 `{haystack}` 中首次出现的位置: {pos}"),
    None => println!("`{needle}` 未在 `{haystack}` 中出现"),
  }

  // kmp_match: 返回所有不重叠的匹配位置
  let text = "abababab";
  let pattern = "abab";
  let all_positions = kmp_match(pattern.as_bytes(), text.as_bytes());
  println!("`{pattern}` 在 `{text}` 中的所有匹配位置: {all_positions:?}");

  // kmp_table: 预生成 LSP（最长后缀-前缀）表，
  // 同一个 needle 搜索多个 haystack 时可以复用，避免重复建表
  let needle2 = "abc";
  let lsp = kmp_table(needle2.as_bytes());
  println!("`{needle2}` 的 LSP 表: {lsp:?}");
  for hay in ["xxabcxx", "abcabc", "no match here"] {
    let found = kmp_find_with_lsp_table(needle2.as_bytes(), hay.as_bytes(), &lsp);
    println!("在 `{hay}` 中查找 `{needle2}`: {found:?}");
  }

  // 泛型切片匹配：不限于字符串，任何实现 PartialEq 的元素切片都可以
  let numbers = [1, 2, 3, 4, 1, 2, 3, 4, 5];
  let sub = [3, 4, 5];
  println!(
    "在 {numbers:?} 中查找子序列 {sub:?}: {:?}",
    kmp_find(&sub, &numbers)
  );
}
