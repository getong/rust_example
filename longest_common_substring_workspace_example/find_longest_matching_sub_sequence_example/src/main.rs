#[allow(dead_code)]
fn find_longest_common(s1: &[char], s2: &[char]) -> Vec<char> {
  if s1.len() > 0 && s2.len() > 0 {
    if !s2.contains(&s1[0]) {
      return find_longest_common(&s1[1 ..], s2);
    }
    if !s1.contains(&s2[0]) {
      return find_longest_common(s1, &s2[1 ..]);
    }
    let s2_match_idx = s2.iter().position(|c| *c == s1[0]).unwrap();
    let s1_match_idx = s1.iter().position(|c| *c == s2[0]).unwrap();
    let mut with_first_of_s1 = vec![s1[0]];
    with_first_of_s1.append(&mut find_longest_common(
      &s1[1 ..],
      &s2[s2_match_idx + 1 ..],
    ));
    let mut with_first_of_s2 = vec![s2[0]];
    with_first_of_s2.append(&mut find_longest_common(
      &s1[s1_match_idx + 1 ..],
      &s2[1 ..],
    ));
    let with_none_of_both = find_longest_common(&s1[1 ..], &s2[1 ..]);
    let max_len = with_first_of_s1
      .len()
      .max(with_first_of_s2.len())
      .max(with_none_of_both.len());
    return if with_first_of_s1.len() == max_len {
      with_first_of_s1
    } else if with_first_of_s2.len() == max_len {
      with_first_of_s2
    } else {
      with_none_of_both
    };
  }
  return vec![];
}

fn main() {
  let s1 = "ABAZDC".chars().collect::<Vec<char>>();
  let s2 = "BACBAD".chars().collect::<Vec<char>>();
  assert_eq!(
    find_longest_common(&s1[..], &s2[..]),
    vec!['A', 'B', 'A', 'D']
  );

  let s1 = "AGGTAB".chars().collect::<Vec<char>>();
  let s2 = "GXTXAYB".chars().collect::<Vec<char>>();
  assert_eq!(
    find_longest_common(&s1[..], &s2[..]),
    vec!['G', 'T', 'A', 'B']
  );
}
