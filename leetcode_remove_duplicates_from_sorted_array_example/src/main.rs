struct Solution;

impl Solution {
  pub fn remove_duplicates(nums: &mut Vec<i32>) -> i32 {
    if nums.len() == 0 {
      return 0;
    }

    let mut i = 0;
    for j in 1..nums.len() {
      if nums[i] != nums[j] {
        // if j - i > 1 {
        nums[i + 1] = nums[j];
        // }
        i += 1;
      }
    }
    (i + 1) as i32
  }
}

fn main() {
  // println!("Hello, world!");

  let mut nums1: Vec<i32> = vec![1, 1, 2];
  let len1 = Solution::remove_duplicates(&mut nums1);
  assert_eq!(len1, 2);
  assert_eq!((&nums1[0..len1 as usize]).to_vec(), vec![1_i32, 2]);

  let mut nums2: Vec<i32> = vec![0, 0, 1, 1, 1, 2, 2, 3, 3, 4];
  let len2 = Solution::remove_duplicates(&mut nums2);
  assert_eq!(len2, 5);
  assert_eq!((&nums2[0..len2 as usize]).to_vec(), vec![0_i32, 1, 2, 3, 4]);
}
