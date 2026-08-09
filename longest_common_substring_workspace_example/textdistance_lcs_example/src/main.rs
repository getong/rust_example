use textdistance::{Algorithm, LCSSeq, LCSStr, nstr, str as td_str};

/// `textdistance` 提供两个 LCS 算法（注意：它是相似度库，只返回长度/相似度，
/// 不返回 LCS 内容本身）：
/// - `LCSSeq`：最长公共子序列（可不连续），O(n*m) DP；
/// - `LCSStr`：最长公共子串（必须连续），O(n*m) DP。
fn main() {
  let (a, b) = ("abcdef", "xbcegf");

  // 1) 快捷函数：直接返回长度
  println!("lcsseq({a:?}, {b:?}) = {}", td_str::lcsseq(a, b)); // 4，即 "bcef"
  println!("lcsstr({a:?}, {b:?}) = {}", td_str::lcsstr(a, b)); // 2，即 "bc"

  // 2) 归一化版本：返回 [0, 1] 的相似度（除以较长串的长度）
  println!("nstr::lcsseq = {:.3}", nstr::lcsseq(a, b));
  println!("nstr::lcsstr = {:.3}", nstr::lcsstr(a, b));

  // 3) Algorithm trait 完整接口：Result 同时携带相似度与距离两种视角
  let r = LCSSeq::default().for_str(a, b);
  println!(
    "\nfor_str: val={} sim={} dist={} nsim={:.3} ndist={:.3}",
    r.val(),   // 原始值：LCS 长度 4
    r.sim(),   // 相似度视角：同 val
    r.dist(),  // 距离视角：max_len - val = 2
    r.nsim(),  // 归一化相似度
    r.ndist()  // 归一化距离
  );

  // 4) 不止字符级：for_words 按词比较，for_vec 对任意 Eq 切片比较
  let (s1, s2) = ("the quick brown fox", "the slow brown dog");
  println!(
    "\nfor_words: {} 个公共词",
    LCSSeq::default().for_words(s1, s2).val()
  );
  let v = LCSStr::default().for_vec(&[1, 2, 3, 4, 5], &[9, 2, 3, 4, 8]);
  println!("for_vec:   连续公共段长 {}", v.val());

  // 5) Unicode 按字符（char）而非字节计数
  println!(
    "\nlcsseq(中文) = {}",
    td_str::lcsseq("人类基因组计划", "小鼠基因组测序")
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn doc_examples() {
    assert_eq!(td_str::lcsseq("abcdef", "xbcegf"), 4); // "bcef"
    assert_eq!(td_str::lcsstr("abcdef", "xbcegf"), 2); // "bc"
  }

  #[test]
  fn subsequence_vs_substring() {
    // 子串必连续，所以 lcsstr <= lcsseq 恒成立
    let cases = [
      ("ABCBDAB", "BDCABA"),
      ("mississippi", "pipimiss"),
      ("same", "same"),
      ("abc", "xyz"),
    ];
    for (a, b) in cases {
      assert!(
        td_str::lcsstr(a, b) <= td_str::lcsseq(a, b),
        "case ({a}, {b})"
      );
    }
    assert_eq!(td_str::lcsseq("ABCBDAB", "BDCABA"), 4); // CLRS 例子
  }

  #[test]
  fn result_views_consistent() {
    let r = LCSSeq::default().for_str("abcdef", "xbcegf");
    // sim + dist = 较长串长度；nsim + ndist = 1
    assert_eq!(r.sim() + r.dist(), 6);
    assert!((r.nsim() + r.ndist() - 1.0).abs() < 1e-9);
  }

  #[test]
  fn unicode_counts_chars() {
    // 公共子序列 "基因组"：按 char 计 3，而非按字节计 9
    assert_eq!(td_str::lcsseq("人类基因组计划", "小鼠基因组测序"), 3);
  }

  #[test]
  fn words_and_vec() {
    assert_eq!(
      LCSSeq::default()
        .for_words("the quick brown fox", "the slow brown dog")
        .val(),
      2
    );
    assert_eq!(
      LCSStr::default()
        .for_vec(&[1, 2, 3, 4, 5], &[9, 2, 3, 4, 8])
        .val(),
      3
    );
  }

  #[test]
  fn edge_cases() {
    assert_eq!(td_str::lcsseq("", "abc"), 0);
    assert_eq!(td_str::lcsstr("", ""), 0);
    assert_eq!(td_str::lcsseq("same", "same"), 4);
    assert_eq!(nstr::lcsseq("same", "same"), 1.0);
    assert_eq!(nstr::lcsseq("", ""), 1.0); // 空串视为完全相似
  }
}
