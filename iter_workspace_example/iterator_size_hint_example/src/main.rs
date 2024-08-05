fn main() {
  let range = 0 ..;
  let tuple = range.size_hint();
  println!("tuple: {:?}", tuple);

  let range2 = 0 .. 10;
  let tuple2 = range2.size_hint();
  println!("tuple2: {:?}", tuple2);
}
