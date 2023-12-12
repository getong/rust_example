fn main() {
  let data = vec![1, 2, 3];

  //let ref_shared = &data[0]; // ref_shared是「引⽤」，详情⻅后
  // let pointer_const: *const u8 = ref_shared; // 隐式转换

  // 精简为：
  let pointer_const: *const u8 = &data[0];

  println!("address of p_const: {:p}", pointer_const);
  unsafe {
    println!("data at p_const: {}", *pointer_const);
  }
}
