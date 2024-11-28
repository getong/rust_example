fn main() {
  // let new_nums: Vec<i32> = (0..10).filter(|&n| *n % 2 == 0).collect();
  // println!("new_nums:{:?}", new_nums);
  let new_nums1: Vec<i32> = (0 .. 10).filter(|&n| n % 2 == 0).collect();
  println!("new_nums1:{:?}", new_nums1);

  let new_nums2: Vec<i32> = (0 .. 10).filter(|n| *n % 2 == 0).collect();
  println!("new_nums2:{:?}", new_nums2);

  let new_nums3: Vec<i32> = (0 .. 10).filter(|n| n % 2 == 0).collect();
  println!("new_nums3:{:?}", new_nums3);
}
