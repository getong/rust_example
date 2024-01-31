fn kmp_search(text: &str, pattern: &str) -> Vec<usize> {
  let (text, pattern) = (text.as_bytes(), pattern.as_bytes());
  let (n, m) = (text.len(), pattern.len());
  let mut result = Vec::new();
  if m == 0 {
    return result; // Empty pattern
  }

  // Preprocess the pattern to get the longest prefix that is also a suffix
  let lps = compute_lps(pattern);

  let (mut i, mut j) = (0, 0); // indexes for text and pattern
  while i < n {
    if pattern[j] == text[i] {
      i += 1;
      j += 1;
    }
    if j == m {
      // Found a match
      result.push(i - j);
      j = lps[j - 1]; // Continue search for next match
    } else if i < n && pattern[j] != text[i] {
      if j != 0 {
        j = lps[j - 1];
      } else {
        i += 1;
      }
    }
  }
  result
}

// Compute Longest Prefix Suffix (LPS) array
fn compute_lps(pattern: &[u8]) -> Vec<usize> {
  let m = pattern.len();
  let mut lps = vec![0; m];
  let mut len = 0; // length of the previous longest prefix suffix
  let mut i = 1;

  while i < m {
    if pattern[i] == pattern[len] {
      len += 1;
      lps[i] = len;
      i += 1;
    } else {
      if len != 0 {
        len = lps[len - 1];
        // Note that we do not increment i here
      } else {
        lps[i] = 0;
        i += 1;
      }
    }
  }
  lps
}

fn main() {
  let text = "ABABDABACDABABCABAB";
  let pattern = "ABABCABAB";
  let matches = kmp_search(text, pattern);
  println!("Pattern found at positions: {:?}", matches);
}
