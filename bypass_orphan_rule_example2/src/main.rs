#[derive(Debug)]
pub struct MyStruct {
  pub x: i32,
  pub y: i32,
}

#[derive(Debug)]
pub struct MyOtherStruct {
  pub a: i32,
  pub b: i32,
}

impl From<MyStruct> for MyOtherStruct {
  fn from(s: MyStruct) -> MyOtherStruct {
    MyOtherStruct { a: s.x, b: s.y }
  }
}

fn main() {
  // println!("Hello, world!");
  let my_struct = MyStruct { x: 1, y: 2 };
  // Using From trait
  let my_other_struct: MyOtherStruct = my_struct.into();
  println!("my_other_struct:{:?}", my_other_struct);
}
