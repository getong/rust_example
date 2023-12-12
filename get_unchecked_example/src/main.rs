fn main() {
  // println!("Hello, world!");
  let arr = ['a', 'b', 'c', 'd', 'e', 'f'];

  for i in 0..arr.len() {
    println!("i is {}", arr[i]);
  }

  // more efficient than the above code
  for c in &arr {
    println!("c is {}", c);
  }

  let arr2 = &arr;
  unsafe {
    println!("get_unchecked : {:?} ", arr2.get_unchecked(100));
  }
}
