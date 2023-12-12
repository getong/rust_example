use std::convert::TryFrom;

#[derive(Debug)]
struct GreaterThanZero(i32);

impl TryFrom<i32> for GreaterThanZero {
  type Error = &'static str;

  fn try_from(value: i32) -> Result<Self, Self::Error> {
    if value <= 0 {
      Err("GreaterThanZero only accepts value superior than zero!")
    } else {
      Ok(GreaterThanZero(value))
    }
  }
}

fn main() {
  // println!("Hello, world!");
  let num = i32::try_from(2_i32).unwrap();
  println!("num : {:?}", num);
}
