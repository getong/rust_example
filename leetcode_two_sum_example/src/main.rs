use std::collections::HashMap;

struct Solution;

impl Solution {
  pub fn two_sum(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let mut map: HashMap<i32, usize> = HashMap::new();

    for i in 0..nums.len() {
      let complement = target - nums[i];
      if map.contains_key(&complement) {
        return vec![map[&complement] as i32, i as i32];
      }

      map.insert(nums[i], i);
    }
    return vec![];
  }
}

fn main() {
  // println!("Hello, world!");

  assert_eq!(Solution::two_sum(vec![2, 7, 11, 15], 9), vec![0, 1]);
  assert_eq!(Solution::two_sum(vec![2, 6, 11, 7], 9), vec![0, 3]);
}
