#![feature(label_break_value)]

fn main() {
  // println!("Hello, world!");
  let container = [0_i32, -1, 1];
  let result = 'block: {
    for &v in container.iter() {
      if v > 0 {
        break 'block v;
      }
    }
    0
  };

  assert_eq!(result, 1);
}
