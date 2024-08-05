pub fn build_heap_down_up(nums: &mut Vec<i32>) {
  for i in 1 .. nums.len() {
    heapify_down_up(nums, i);
  }
}

fn heapify_down_up(nums: &mut Vec<i32>, idx: usize) {
  let mut idx = idx;
  let mut parent_idx = (idx - 1) / 2;
  while nums[idx] > nums[parent_idx] {
    nums.swap(idx, parent_idx);
    idx = parent_idx;
    if idx == 0 {
      break;
    }
    parent_idx = (idx - 1) / 2;
  }
}

pub fn build_heap_up_down(nums: &mut Vec<i32>) {
  let len = nums.len();
  for i in (0 .. len / 2).rev() {
    heapify_up_down(nums, i, len);
  }
}

fn heapify_up_down(nums: &mut Vec<i32>, idx: usize, nums_len: usize) {
  let mut idx = idx;
  loop {
    let mut max_pos = idx;
    if 2 * idx + 1 < nums_len && nums[idx] < nums[2 * idx + 1] {
      max_pos = 2 * idx + 1;
    }

    if 2 * idx + 2 < nums_len && nums[max_pos] < nums[2 * idx + 2] {
      max_pos = 2 * idx + 2;
    }

    if max_pos == idx {
      break;
    }
    nums.swap(idx, max_pos);
    idx = max_pos;
  }
}

pub fn insert(nums: &mut Vec<i32>, x: i32) -> bool {
  nums.push(x);
  if nums.len() > 1 {
    heapify_down_up(nums, nums.len() - 1);
  }
  true
}

pub fn remove_max(nums: &mut Vec<i32>) -> Option<i32> {
  if nums.len() == 0 {
    return None;
  }
  let max_value = nums[0];
  nums[0] = nums[nums.len() - 1];
  nums.remove(nums.len() - 1);
  if nums.len() > 1 {
    heapify_up_down(nums, 0, nums.len());
  }
  Some(max_value)
}

fn main() {
  // println!("Hello, world!");
  let mut vec1 = vec![1, 2, 3, 4, 5, 6];
  build_heap_down_up(&mut vec1);
  println!("vec1:{:?}", vec1);

  let mut vec2 = vec![1, 2, 3, 4, 5, 6, 7];
  build_heap_down_up(&mut vec2);
  println!("vec2:{:?}", vec2);

  let mut vec3 = vec![1, 2, 3, 4, 5, 6, 7];
  build_heap_up_down(&mut vec3);
  println!("vec3:{:?}", vec3);
}
