use similar::{Algorithm, ChangeTag, DiffOp, TextDiff, capture_diff_slices};

/// 用 `similar` 的经典 LCS 表算法求两个字符串的最长公共子序列。
///
/// `similar` 是 diff 库，不直接暴露"返回 LCS"的接口，但 diff 与 LCS 是同一问题的
/// 两面：最小编辑脚本（只含插入/删除）中所有 Equal 段拼起来就是一条最长公共子序列。
fn lcs_chars(a: &str, b: &str) -> String {
  TextDiff::configure()
    .algorithm(Algorithm::Lcs)
    .diff_chars(a, b)
    .iter_all_changes()
    .filter(|c| c.tag() == ChangeTag::Equal)
    .map(|c| c.value())
    .collect()
}

/// 同样的思路对任意 `T: Eq + Hash` 的切片也成立：`capture_diff_slices` 返回
/// `DiffOp` 列表，取其中 Equal 段覆盖的元素即为 LCS。
fn lcs_slice<T: Clone + Eq + std::hash::Hash + Ord>(a: &[T], b: &[T]) -> Vec<T> {
  capture_diff_slices(Algorithm::Lcs, a, b)
    .iter()
    .filter_map(|op| {
      if let DiffOp::Equal { old_index, len, .. } = *op {
        Some(&a[old_index .. old_index + len])
      } else {
        None
      }
    })
    .flatten()
    .cloned()
    .collect()
}

fn main() {
  // 1) 字符级 LCS
  let (a, b) = ("ABCBDAB", "BDCABA");
  println!("lcs({a:?}, {b:?}) = {:?}", lcs_chars(a, b));

  let (a, b) = ("人类基因组计划草图", "小鼠基因组测序计划");
  println!("lcs({a:?}, {b:?}) = {:?}", lcs_chars(a, b));

  // 2) 任意切片的 LCS（这里用整数序列）
  let (xs, ys) = ([1, 3, 4, 1, 2, 1, 3], [3, 4, 1, 2, 1, 3, 2]);
  println!("lcs({xs:?}, {ys:?}) = {:?}", lcs_slice(&xs, &ys));

  // 3) similar 的本职工作：基于 LCS 的 diff
  let old = "hello world\nfoo bar\nlast line\n";
  let new = "hello there\nfoo bar\nnew line\n";
  let diff = TextDiff::configure()
    .algorithm(Algorithm::Lcs)
    .diff_lines(old, new);
  println!("\nsimilarity ratio = {:.2}", diff.ratio());
  print!(
    "{}",
    diff
      .unified_diff()
      .context_radius(1)
      .header("old.txt", "new.txt")
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 朴素 O(n*m) DP，作为对拍基准（LCS 不唯一，只比长度）
  fn dp_lcs_len(a: &[char], b: &[char]) -> usize {
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0 .. a.len() {
      for j in 0 .. b.len() {
        dp[i + 1][j + 1] = if a[i] == b[j] {
          dp[i][j] + 1
        } else {
          dp[i][j + 1].max(dp[i + 1][j])
        };
      }
    }
    dp[a.len()][b.len()]
  }

  /// 验证结果确实是两边的公共子序列
  fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut it = hay.chars();
    needle.chars().all(|c| it.any(|h| h == c))
  }

  #[test]
  fn classic_textbook_case() {
    // CLRS 教科书例子，LCS 长度为 4（如 "BCBA"）
    let r = lcs_chars("ABCBDAB", "BDCABA");
    assert_eq!(r.chars().count(), 4);
    assert!(is_subsequence(&r, "ABCBDAB") && is_subsequence(&r, "BDCABA"));
  }

  #[test]
  fn agrees_with_naive_dp() {
    let cases = [
      ("ABCBDAB", "BDCABA"),
      ("mississippi", "pipimiss"),
      ("", "abc"),
      ("abc", ""),
      ("same", "same"),
      ("abc", "xyz"),
      ("人类基因组计划草图", "小鼠基因组测序计划"),
    ];
    for (a, b) in cases {
      let r = lcs_chars(a, b);
      let expect = dp_lcs_len(
        &a.chars().collect::<Vec<_>>(),
        &b.chars().collect::<Vec<_>>(),
      );
      assert_eq!(r.chars().count(), expect, "case ({a}, {b}), got {r:?}");
      assert!(
        is_subsequence(&r, a) && is_subsequence(&r, b),
        "case ({a}, {b}), got {r:?}"
      );
    }
  }

  #[test]
  fn slice_version() {
    assert_eq!(
      lcs_slice(&[1, 3, 4, 1, 2, 1, 3], &[3, 4, 1, 2, 1, 3, 2]),
      vec![3, 4, 1, 2, 1, 3]
    );
    assert_eq!(lcs_slice::<i32>(&[], &[1, 2]), vec![]);
  }
}
