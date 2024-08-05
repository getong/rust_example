use std::collections::HashMap;

struct Solution;

impl Solution {
  pub fn group_anagrams(strs: Vec<String>) -> Vec<Vec<String>> {
    let mut vecs: Vec<Vec<String>> = Vec::new();
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for i in 0 .. strs.len() {
      let mut chars = vec![];
      for c in strs[i].chars() {
        chars.push(c);
      }
      chars.sort();

      let key: String = chars.into_iter().collect();

      let value = map.get(&key);
      if value != None {
        let mut v = value.unwrap().to_vec();
        v.push(strs[i].clone());
        map.insert(key, v);
      } else {
        let v = vec![strs[i].clone()];
        map.insert(key, v);
      }
    }

    for val in map.values() {
      vecs.push(val.to_vec());
    }
    return vecs;
  }
}

fn main() {
  // println!("Hello, world!");

  let mut vec1 = Solution::group_anagrams(vec![
    "eat".to_string(),
    "tea".to_string(),
    "tan".to_string(),
    "ate".to_string(),
    "nat".to_string(),
    "bat".to_string(),
  ]);
  vec1.sort();

  let mut vec2 = vec![
    vec!["eat".to_string(), "tea".to_string(), "ate".to_string()],
    vec!["tan".to_string(), "nat".to_string()],
    vec!["bat".to_string()],
  ];
  vec2.sort();

  assert_eq!(vec1, vec2);
}
