fn main() {
  let num1 = vec![1, 2, 3];
  let num2 = vec![3];
  let mut iter1 = num1.iter();
  let mut iter2 = num2.iter();
  if iter1.len() < iter2.len() {
    std::mem::swap(&mut iter1, &mut iter2);
  } // now iter1 is the largest
  for i in iter1.zip(iter2.chain(std::iter::repeat(&0))) {
    println!("{:?}", i);
  }
}
