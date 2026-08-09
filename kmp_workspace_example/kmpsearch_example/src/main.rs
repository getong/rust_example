// 使用 crates.io 上的 `kmpsearch` 库（https://crates.io/crates/kmpsearch）进行 KMP 匹配。
// 与 `kmp`/`kmpm` 的自由函数风格不同，kmpsearch 采用 trait 风格：
// 它定义了 Haystack trait，并为所有能转成 &[u8] 的类型（&str、String、字节切片等）
// 提供了实现，因此可以直接在字符串/字节数据上链式调用搜索方法：
//   - contains_needle:       是否包含 pattern
//   - first_indexof_needle:  第一个匹配的字节下标
//   - last_indexof_needle:   最后一个匹配的字节下标
//   - indexesof_needle:      所有匹配的字节下标（无匹配时返回 None）
use kmpsearch::Haystack;

fn main() {
  let text = "The quick brown fox jumps over the lazy dog, the end.";

  // contains_needle: 布尔判断，类似 str::contains
  println!("文本包含 `fox` 吗: {}", text.contains_needle("fox"));
  println!("文本包含 `cat` 吗: {}", text.contains_needle("cat"));

  // first / last indexof: 首个与最后一个匹配位置
  println!("`the` 首次出现于: {:?}", text.first_indexof_needle("the"));
  println!("`the` 最后出现于: {:?}", text.last_indexof_needle("the"));

  // indexesof_needle: 所有匹配位置，返回 Option<Vec<usize>>，无匹配为 None
  println!("`the` 的所有位置: {:?}", text.indexesof_needle("the"));
  println!("`cat` 的所有位置: {:?}", text.indexesof_needle("cat"));

  // needle 是泛型 N: AsRef<[u8]>，String、&str、&[u8] 都可以直接传
  let needle_string = String::from("quick");
  println!(
    "`quick`（String 形式）首次出现于: {:?}",
    text.first_indexof_needle(&needle_string)
  );

  // haystack 同样不限于 &str：字节切片也实现了 Haystack，
  // 适合在二进制数据中定位特征字节序列
  let binary: &[u8] = &[0x00, 0xde, 0xad, 0xbe, 0xef, 0x00, 0xde, 0xad];
  let sig: &[u8] = &[0xde, 0xad];
  println!(
    "字节特征 {sig:02x?} 在二进制数据中的位置: {:?}",
    binary.indexesof_needle(sig)
  );

  // 匹配语义验证：indexesof_needle 返回的是不重叠的匹配，
  // "aba" 在 "abababa" 中只命中 0 和 4（重叠的位置 2 被跳过）
  println!(
    "`aba` 在 `abababa` 中的所有位置: {:?}",
    "abababa".indexesof_needle("aba")
  );
}
