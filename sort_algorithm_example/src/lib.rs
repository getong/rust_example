use std::collections::VecDeque;

pub fn add(left: usize, right: usize) -> usize {
  left + right
}

pub fn bubble_sort(nums: &mut Vec<usize>) {
  for _i in 1 .. nums.len() {
    for i in 1 .. nums.len() {
      if nums[i - 1] > nums[i] {
        nums.swap(i - 1, i);
      }
    }
  }
}

pub fn insert_sort(nums: &mut Vec<usize>) {
  for i in 1 .. nums.len() {
    let (mut p, v) = (i, nums[i]);
    while p > 0 && nums[p - 1] > v {
      nums[p] = nums[p - 1];
      p -= 1;
    }
    nums[p] = v;
  }
}

pub fn shell_sort(nums: &mut Vec<usize>) {
  fn _insert_sort(nums: &mut Vec<usize>, g: usize) {
    for i in g .. nums.len() {
      let (mut p, v) = (i, nums[i]);
      while p >= g && nums[p - g] > v {
        nums[p] = nums[p - g];
        p -= g;
      }
      nums[p] = v;
    }
  }

  let mut a: VecDeque<usize> = VecDeque::new();
  a.push_front(1);
  while *a.front().unwrap() <= nums.len() {
    a.push_front(3 * a.front().unwrap() + 1);
  }
  for &g in a.iter() {
    _insert_sort(nums, g);
  }
}

pub fn selection_sort(nums: &mut Vec<usize>) {
  for i in 0 .. nums.len() - 1 {
    let mut p = i;
    for j in i + 1 .. nums.len() {
      if nums[j] < nums[p] {
        p = j;
      }
    }
    nums.swap(i, p);
  }
}

pub fn count_sort(nums: &mut Vec<usize>) {
  let n = nums.iter().max().unwrap();
  let origin_nums = nums.clone();
  let mut count: Vec<usize> = Vec::new();
  for _i in 0 .. n + 1 {
    count.push(0)
  }
  for &v in nums.iter() {
    count[v] += 1;
  }
  for i in 1 .. count.len() {
    count[i] += count[i - 1];
  }
  for &v in origin_nums.iter() {
    nums[count[v] - 1] = v;
    count[v] -= 1;
  }
}

pub fn quick_sort(nums: &mut Vec<usize>) {
  fn _partition(nums: &mut Vec<usize>, begin: usize, end: usize) -> usize {
    let (mut i, v) = (begin, nums[end - 1]);
    for j in begin .. end - 1 {
      if nums[j] <= v {
        nums.swap(i, j);
        i += 1;
      }
    }
    nums.swap(i, end - 1);
    i
  }

  fn _quick_sort(nums: &mut Vec<usize>, begin: usize, end: usize) {
    if begin + 1 < end {
      let mid = _partition(nums, begin, end);
      _quick_sort(nums, begin, mid);
      _quick_sort(nums, mid + 1, end);
    }
  }

  _quick_sort(nums, 0, nums.len())
}

pub fn merge_sort(nums: &mut Vec<usize>) {
  fn _merge(nums: &mut Vec<usize>, left: usize, mid: usize, right: usize) {
    let left_part: Vec<usize> = nums[left .. mid].iter().cloned().collect();
    let right_part: Vec<usize> = nums[mid .. right].iter().cloned().collect();
    let (mut left_offset, mut right_offset) = (0usize, 0usize);
    while left_offset < left_part.len() || right_offset < right_part.len() {
      if right_offset == right_part.len()
        || (left_offset < left_part.len() && left_part[left_offset] <= right_part[right_offset])
      {
        nums[left + left_offset + right_offset] = left_part[left_offset];
        left_offset += 1;
      } else {
        nums[left + left_offset + right_offset] = right_part[right_offset];
        right_offset += 1;
      }
    }
  }

  fn _merge_sort(nums: &mut Vec<usize>, left: usize, right: usize) {
    if left + 1 < right {
      let mid = (left + right) / 2;
      _merge_sort(nums, left, mid);
      _merge_sort(nums, mid, right);
      _merge(nums, left, mid, right);
    }
  }

  _merge_sort(nums, 0, nums.len())
}

pub struct Heap<T: Ord> {
  elems: Vec<T>, // 保存完全二叉树
}

impl<T: Ord> Heap<T> {
  pub fn new() -> Heap<T> {
    Heap { elems: Vec::new() }
  }

  // 从向量创建一个最大堆
  pub fn from(elems: Vec<T>) -> Heap<T> {
    let mut heap = Heap { elems: elems };
    // 自底向上遍历非叶节点
    for i in (0 .. heap.len() / 2).rev() {
      // 下沉节点i
      heap.max_heapify(i)
    }
    heap
  }

  // 计算父节点下标
  pub fn parent(i: usize) -> usize {
    if i > 0 {
      (i - 1) / 2
    } else {
      0
    }
  }

  // 计算左子节点下标
  pub fn left(i: usize) -> usize {
    i * 2 + 1
  }

  // 计算右子节点下标
  pub fn right(i: usize) -> usize {
    i * 2 + 2
  }

  // 对节点i进行下沉操作
  pub fn max_heapify(&mut self, i: usize) {
    let (left, right, mut largest) = (Heap::<T>::left(i), Heap::<T>::right(i), i);
    if left < self.len() && self.elems[left] > self.elems[largest] {
      largest = left;
    }
    if right < self.len() && self.elems[right] > self.elems[largest] {
      largest = right;
    }
    if largest != i {
      self.elems.swap(largest, i);
      self.max_heapify(largest);
    }
  }

  // 插入一个元素
  pub fn push(&mut self, v: T) {
    self.elems.push(v);
    // 上升元素
    let mut i = self.elems.len() - 1;
    while i > 0 && self.elems[Heap::<T>::parent(i)] < self.elems[i] {
      self.elems.swap(i, Heap::<T>::parent(i));
      i = Heap::<T>::parent(i);
    }
  }

  // 弹出最大元素
  pub fn pop(&mut self) -> Option<T> {
    if self.is_empty() {
      None
    } else {
      let b = self.elems.len() - 1;
      self.elems.swap(0, b);
      let v = Some(self.elems.pop().unwrap());
      if !self.is_empty() {
        // 下沉根节点
        self.max_heapify(0);
      }
      v
    }
  }

  pub fn is_empty(&self) -> bool {
    self.elems.is_empty()
  }

  pub fn len(&self) -> usize {
    self.elems.len()
  }
}

pub fn heap_sort(nums: &mut Vec<usize>) {
  let mut heap: Heap<usize> = Heap::from(nums.clone());
  for i in (0 .. nums.len()).rev() {
    nums[i] = heap.pop().unwrap();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
  }
}
