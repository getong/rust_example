use smallvec::{smallvec, SmallVec};

fn main() {
  // println!("Hello, world!");
  // This SmallVec can hold up to 4 items on the stack:
  let mut v: SmallVec<[i32; 4]> = smallvec![1, 2, 3, 4];

  // It will automatically move its contents to the heap if
  // contains more than four items:
  v.push(5);

  println!("init v: {:?}", v);

  // SmallVec points to a slice, so you can use normal slice
  // indexing and other methods to access its contents:
  v[0] = v[1] + v[2];
  v.sort();
  println!("v: {:?}", v);
}
