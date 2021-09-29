use std::collections::HashMap;

struct Solution;

impl Solution {
    pub fn group_anagrams(strs: Vec<String>) -> Vec<Vec<String>> {
        let mut vecs: Vec<Vec<String>> = Vec::new();
        let mut used: Vec<bool> = vec![false; strs.len()];

        for i in 0..strs.len() {
            let mut temp: Vec<String> = Vec::new();
            if !used[i] {
                temp.push(strs[i].clone());
                for j in i + 1..strs.len() {
                    let mut is_anagram: bool = true;
                    if strs[i].len() != strs[j].len() {
                        continue;
                    }

                    let mut map = HashMap::new();
                    for c in strs[i].chars() {
                        let count = map.entry(c).or_insert(0);
                        *count += 1;
                    }

                    for c in strs[j].chars() {
                        let count = map.entry(c).or_insert(0);
                        *count -= 1;

                        if *count < 0 {
                            is_anagram = false;
                            break;
                        }
                    }
                    if is_anagram {
                        used[j] = true;
                        temp.push(strs[j].clone());
                    }
                }
            }
            if !temp.is_empty() {
                vecs.push(temp);
            }
        }
        return vecs;
    }
}

fn main() {
    // println!("Hello, world!");

    assert_eq!(
        Solution::group_anagrams(vec![
            "eat".to_string(),
            "tea".to_string(),
            "tan".to_string(),
            "ate".to_string(),
            "nat".to_string(),
            "bat".to_string()
        ]),
        vec![
            vec!["eat".to_string(), "tea".to_string(), "ate".to_string()],
            vec!["tan".to_string(), "nat".to_string()],
            vec!["bat".to_string()]
        ]
    );
}
