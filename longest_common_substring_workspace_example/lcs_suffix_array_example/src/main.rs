fn suffix_array(s: &str) -> Vec<usize> {
  let s = s.as_bytes();
  let n = s.len();
  let mut sa: Vec<usize> = (0 .. n).collect();
  let mut rank: Vec<usize> = s.iter().map(|&x| x as usize).collect();
  let mut tmp = vec![0; n];
  let mut k = 1;
  while k < n {
    sa.sort_by(|&i, &j| {
      if rank[i] != rank[j] {
        rank[i].cmp(&rank[j])
      } else {
        let ri = if i + k < n { rank[i + k] } else { 0 };
        let rj = if j + k < n { rank[j + k] } else { 0 };
        ri.cmp(&rj)
      }
    });
    tmp[sa[0]] = 0;
    for i in 0 .. n {
      tmp[sa[i]] = tmp[sa[i - 1]]
        + (if rank[sa[i]] != rank[sa[i - 1]] || rank[sa[i] + k] != rank[sa[i - 1] + k] {
          1
        } else {
          0
        });
    }
    std::mem::swap(&mut rank, &mut tmp);
    k *= 2;
  }
  sa
}

fn lcp_array(s: &str, sa: &[usize]) -> Vec<usize> {
  let s = s.as_bytes();
  let n = s.len();
  let mut rank = vec![0; n];
  let mut lcp = vec![0; n - 1];
  for i in 0 .. n {
    rank[sa[i]] = i;
  }
  let mut h = 0;
  for i in 0 .. n {
    if h > 0 {
      h -= 1;
    }
    if rank[i] == 0 {
      continue;
    }
    let j = sa[rank[i] - 1];
    while i + h < n && j + h < n && s[i + h] == s[j + h] {
      h += 1;
    }
    lcp[rank[i] - 1] = h;
  }
  lcp
}

fn longest_common_substring(s1: &str, s2: &str) -> String {
  let concat = format!("{}#{}", s1, s2); // Use '#' as a separator.
  let sa = suffix_array(&concat);
  let lcp = lcp_array(&concat, &sa);
  let (mut max_len, mut max_pos) = (0, 0);

  for i in 0 .. lcp.len() {
    if (sa[i] < s1.len()) != (sa[i + 1] < s1.len()) && lcp[i] > max_len {
      max_len = lcp[i];
      max_pos = sa[i];
    }
  }

  concat[max_pos .. max_pos + max_len].to_string()
}

fn main() {
  let s1 = "abcdfgh";
  let s2 = "abdfg";
  let lcs = longest_common_substring(&s1, &s2);
  println!("Longest Common Substring: {}", lcs);
}
