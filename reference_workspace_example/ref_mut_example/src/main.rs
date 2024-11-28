use std::ptr;

fn main() {
  // 定义可变引用
  let mut x = 5;
  // let mut_ref = &mut x; //等价于 let ref mut mut_ref=x;
  let ref mut mut_ref = x;
  *mut_ref = 6;
  // let mut_ref2 = &mut_ref;

  // 定义共享引用
  let y: i32 = 100;
  // let shared_ref = &y; //等价于 let ref shared_ref =y;
  let ref shared_ref = y;
  let shared_ref2 = &shared_ref;

  // 解引用
  println!(
    "value:y={},shared_ref={},shared_ref2={}",
    y, shared_ref, shared_ref2
  ); // 自动解引用
  println!(
    "value:y={},*shared_ref={},**shared_ref2={}",
    y, *shared_ref, **shared_ref2
  ); // 显式解引用
     // println!("***shared_ref2={}",***shared_ref2);//错误的解引用:(type `i32` cannot be dereferenced)

  // 引用之间的关系
  println!(
    "address:y={:p},shared_ref={:p},shared_ref2{:p}",
    &y, &shared_ref, &shared_ref2
  );
  println!(
    "point to:shared_ref=>{:p},shared_ref2=>{:p}",
    shared_ref, shared_ref2
  );
  assert!(ptr::eq(&y, shared_ref));
  assert!(ptr::eq(&shared_ref, shared_ref2));
}
