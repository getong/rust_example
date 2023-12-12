fn main() {
  // println!("Hello, world!");
  let mr: &mut Vec<u32> = &mut vec![];
  {
    // let a = &mut     *mr   ;
    // `*mr` is an lvalue of type `Vec<u32>`
    //  `&mut     *mr` together it's of type `&mut Vec<u32>` again
    let &mut ref mut a = mr;
    a.push(3);
  }
  mr.push(4);
  println!("mr is {:#?}", mr);
}
