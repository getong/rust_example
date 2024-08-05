struct Solution;

impl Solution {
  pub fn move_zeroes(nums: &mut Vec<i32>) {
    let mut i = 0;
    let mut j = 0;
    while j < nums.len() {
      if nums[j] != 0 {
        nums[i] = nums[j];
        i += 1;
      }
      j += 1;
    }

    // while i < nums.len() {
    //    nums[i] = 00;
    //    i += 1;
    // }
    for k in i .. nums.len() {
      nums[k] = 0;
    }
  }
}

fn main() {
  // println!("Hello, world!");
  let mut vec: Vec<i32> = vec![0, 1, 0, 3, 12];
  Solution::move_zeroes(&mut vec);
  println!("{:?}", vec);
}
