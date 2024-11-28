// Blog post: https://creativcoder.dev/merge-k-sorted-arrays-rust

// Merge 2 sorted arrays
pub fn merge_2(a: &[i32], b: &[i32]) -> Vec<i32> {
  let (mut i, mut j) = (0, 0);
  let mut sorted = vec![];
  let remaining;
  let remaining_idx;
  loop {
    if a[i] < b[j] {
      sorted.push(a[i]);
      i += 1;
      if i == a.len() {
        remaining = b;
        remaining_idx = j;
        break;
      }
    } else {
      sorted.push(b[j]);
      j += 1;
      if j == b.len() {
        remaining = a;
        remaining_idx = i;
        break;
      }
    }
  }
  for i in remaining_idx .. remaining.len() {
    sorted.push(remaining[i]);
  }

  sorted
}

// Merge k sorted arrays

#[derive(Debug, Eq)]
struct Item<'a> {
  arr: &'a Vec<i32>,
  idx: usize,
}

impl<'a> PartialEq for Item<'a> {
  fn eq(&self, other: &Self) -> bool {
    self.get_item() == other.get_item()
  }
}

impl<'a> PartialOrd for Item<'a> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.get_item().partial_cmp(&other.get_item())
  }
}

impl<'a> Ord for Item<'a> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.get_item().cmp(&other.get_item())
  }
}

impl<'a> Item<'a> {
  fn new(arr: &'a Vec<i32>, idx: usize) -> Self {
    Self { arr, idx }
  }

  fn get_item(&self) -> i32 {
    self.arr[self.idx]
  }
}

use std::{
  cmp::{Ordering, Reverse},
  collections::BinaryHeap,
};

fn merge(arrays: Vec<Vec<i32>>) -> Vec<i32> {
  let mut sorted = vec![];

  let mut heap = BinaryHeap::with_capacity(arrays.len());
  for arr in &arrays {
    let item = Item::new(arr, 0);
    heap.push(Reverse(item));
  }

  while !heap.is_empty() {
    let mut it = heap.pop().unwrap();
    sorted.push(it.0.get_item());
    it.0.idx += 1;
    if it.0.idx < it.0.arr.len() {
      heap.push(it)
    }
  }

  sorted
}

fn main() {
  let a = vec![1, 5, 7];
  let b = vec![-2, 3, 4];
  let v = vec![a, b];
  dbg!(merge(v));
}
