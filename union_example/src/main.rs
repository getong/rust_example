#[repr(C)]
union Metric {
  rounded: u32,
  precise: f32,
}

union FloatOrInt {
  f: f32,
  i: i32,
}

fn main() {
  // println!("Hello, world!");
  let mut a = Metric { rounded: 323 };
  unsafe {
    println!("rounded: {}", a.rounded);
  }

  unsafe {
    println!("precise: {}", a.precise);
  }

  a.precise = 33.3;
  unsafe {
    println!("precise: {}", a.precise);
  }

  unsafe {
    println!("rounded: {}", a.rounded);
  }

  let mut one = FloatOrInt { i: 1 };
  assert_eq!(unsafe { one.i }, 0x00_00_00_01);
  one.f = 1.0;
  assert_eq!(unsafe { one.i }, 0x3F_80_00_00);
}
