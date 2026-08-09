// 使用 crates.io 上的 `kmpm` 库（https://crates.io/crates/kmpm）进行 KMP 字符串匹配。
// 该库面向 &str 提供三个入口函数，区别在于返回单个/全部匹配、以及是否允许重叠：
//   - kmpm_str:     返回第一个匹配位置
//   - kmpm_str_all: 返回所有匹配位置，允许重叠（duplicates）
//   - kmpm_str_nad: 返回所有匹配位置，不允许重叠（nad = not allow duplicates），
//     并可指定起始搜索下标 cursor_start
use kmpm::{kmpm_str, kmpm_str_all, kmpm_str_nad};

fn main() {
  // kmpm_str: 只找第一个匹配
  let text = "hello world, hello kmp!";
  let pattern = "hello";
  match kmpm_str(text, pattern) {
    Some(pos) => println!("`{pattern}` 在 `{text}` 中首次出现的位置: {pos}"),
    None => println!("`{pattern}` 未在 `{text}` 中出现"),
  }

  // kmpm_str_all: 找出所有匹配，允许重叠
  // "aba" 在 "abababa" 中重叠出现于 0、2、4
  let text2 = "abababa";
  let pattern2 = "aba";
  let all = kmpm_str_all(text2, pattern2);
  println!("`{pattern2}` 在 `{text2}` 中的所有匹配（允许重叠）: {all:?}");

  // kmpm_str_nad: 找出所有不重叠的匹配，第三个参数是起始搜索下标
  // 同样的输入只会命中 0 和 4（位置 2 的匹配与位置 0 的重叠，被跳过）
  // 注意：0.2.2 版本的 kmpm_str_nad 内部残留了调试用的 println!（cursor/skipstep 等），
  // 会混入标准输出，生产环境慎用该函数
  let nad = kmpm_str_nad(text2, pattern2, 0);
  println!("`{pattern2}` 在 `{text2}` 中的所有匹配（不重叠）: {nad:?}");

  // cursor_start 用于跳过文本开头的一段：从下标 1 开始搜索
  let nad_from_1 = kmpm_str_nad(text2, pattern2, 1);
  println!("`{pattern2}` 在 `{text2}` 中从下标 1 开始的不重叠匹配: {nad_from_1:?}");

  // 中文文本同样适用。kmpm 内部按 char 迭代，返回的是「字符」偏移而非字节偏移：
  // `天气` 命中字符下标 2 和 7（若按字节偏移则是 6 和 21）
  let zh_text = "今天天气不错，天气很好";
  let zh_pattern = "天气";
  println!(
    "`{zh_pattern}` 在 `{zh_text}` 中的所有匹配: {:?}",
    kmpm_str_all(zh_text, zh_pattern)
  );
}
