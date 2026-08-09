use stringzilla::sz;

/// 求 `a` 与 `b` 的最长公共子串（按字节比较），返回它在 `a` 中的切片。
///
/// stringzilla 本身不提供 LCS 接口，但它的 `sz::find` 是 SIMD 加速的精确子串搜索。
/// 这里用「二分答案长度 + 滑动窗口」组合：
/// - 若存在长度为 L 的公共子串，则必然存在长度为 L-1 的（取其前缀），所以答案长度可二分；
/// - 对每个候选长度 L，枚举较短串的所有长度 L 窗口，用 `sz::find` 在较长串中查找。
///
/// 每次判定最坏 O(n*m)，共 O(log min(n,m)) 次判定，但内层搜索由 SIMD 加速，实际很快。
fn longest_common_substring<'a>(a: &'a [u8], b: &[u8]) -> &'a [u8] {
  // 窗口在较短串上滑动，搜索在较长串上进行
  let (short, long, short_is_a) = if a.len() <= b.len() {
    (a, b, true)
  } else {
    (b, a, false)
  };

  // 返回较短串中第一个能在较长串里找到的长度为 len 的窗口起点
  let match_at = |len: usize| -> Option<usize> {
    if len == 0 {
      return Some(0);
    }
    (0 .. short.len() + 1 - len).find(|&i| sz::find(long, &short[i .. i + len]).is_some())
  };

  // 二分最大可行长度：lo 恒可行，hi 恒不可行
  let (mut lo, mut hi) = (0, short.len() + 1);
  while lo + 1 < hi {
    let mid = (lo + hi) / 2;
    if match_at(mid).is_some() {
      lo = mid;
    } else {
      hi = mid;
    }
  }

  let i = match_at(lo).unwrap();
  if short_is_a {
    &a[i .. i + lo]
  } else {
    // 结果要求是 a 的切片，把 b 中找到的窗口重新定位回 a
    let j = sz::find(a, &short[i .. i + lo]).unwrap();
    &a[j .. j + lo]
  }
}

fn show(a: &str, b: &str) {
  let lcs = longest_common_substring(a.as_bytes(), b.as_bytes());
  println!(
    "a = {:?}\nb = {:?}\nlcs = {:?} (len = {})\n",
    a,
    b,
    String::from_utf8_lossy(lcs),
    lcs.len()
  );
}

fn main() {
  // sz::find 基本用法：SIMD 加速的精确子串搜索，等价于 memchr::memmem::find
  assert_eq!(sz::find("hello, world", "world"), Some(7));
  assert_eq!(sz::find("hello, world", "rust"), None);

  show(
    "the quick brown fox jumps over the lazy dog",
    "a quick brown cat sleeps under the lazy tree",
  );
  show("abcdefghijklmn", "xyzdefghiuvw");
  show("互联网上的长公共子串示例", "另一个公共子串示例文本");
  show("abc", "xyz");
}

#[cfg(test)]
mod tests {
  use super::*;

  fn lcs_str<'a>(a: &'a str, b: &str) -> &'a str {
    str::from_utf8(longest_common_substring(a.as_bytes(), b.as_bytes())).unwrap()
  }

  #[test]
  fn basic() {
    assert_eq!(lcs_str("abcdef", "zcdemn"), "cde");
  }

  #[test]
  fn no_common() {
    assert_eq!(lcs_str("abc", "xyz"), "");
  }

  #[test]
  fn identical() {
    assert_eq!(lcs_str("same", "same"), "same");
  }

  #[test]
  fn empty_input() {
    assert_eq!(lcs_str("", "anything"), "");
    assert_eq!(lcs_str("anything", ""), "");
  }

  #[test]
  fn common_at_ends() {
    assert_eq!(lcs_str("prefix_tail", "tail"), "tail");
    assert_eq!(lcs_str("head", "head_suffix"), "head");
  }

  #[test]
  fn longer_in_first_arg() {
    // 较短串是 b，验证结果切片正确定位回 a
    let a = "xxlongestcommonxx";
    let r = lcs_str(a, "longestcommon");
    assert_eq!(r, "longestcommon");
    let start = r.as_ptr() as usize - a.as_ptr() as usize;
    assert_eq!(start, 2);
  }

  #[test]
  fn agrees_with_naive_dp() {
    // 与朴素 DP 对拍，只比较长度（同长度的公共子串可能不唯一）
    let naive_len = |a: &[u8], b: &[u8]| -> usize {
      let mut best = 0;
      let mut dp = vec![0usize; b.len() + 1];
      for i in 0 .. a.len() {
        for j in (0 .. b.len()).rev() {
          dp[j + 1] = if a[i] == b[j] { dp[j] + 1 } else { 0 };
          best = best.max(dp[j + 1]);
        }
      }
      best
    };
    let cases = [
      ("the quick brown fox", "a quick brown cat"),
      ("mississippi", "ppississim"),
      ("aaaaab", "baaaaa"),
      ("abcabcabc", "cabcab"),
    ];
    for (a, b) in cases {
      assert_eq!(
        lcs_str(a, b).len(),
        naive_len(a.as_bytes(), b.as_bytes()),
        "case ({a}, {b})"
      );
    }
  }
}
