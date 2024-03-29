struct Solution;

impl Solution {
  pub fn plus_one(mut digits: Vec<i32>) -> Vec<i32> {
    let mut i = digits.len() - 1;
    loop {
      if digits[i] < 9 {
        digits[i] += 1;
        return digits;
      }

      digits[i] = 0;
      if i > 0 {
        i -= 1;
      } else if i == 0 {
        break;
      }
    }

    digits = vec![0; digits.len() + 1];
    digits[0] = 1;
    return digits;
  }
}

fn main() {
  // println!("Hello, world!");
  assert_eq!(Solution::plus_one(vec![1, 2, 3]), vec![1, 2, 4]);
  assert_eq!(Solution::plus_one(vec![9, 9, 9]), vec![1, 0, 0, 0]);
}
