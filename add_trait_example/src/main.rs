use std::ops::Add;

const MAX_COUNTER: i32 = 1024;

struct Counter(i32);

impl Add for Counter {
  type Output = Counter;

  fn add(self, rhs: Self) -> Self::Output {
    Self(self.0 + rhs.0)
  }
}

impl Add for &Counter {
  type Output = Counter;

  fn add(self, rhs: Self) -> Self::Output {
    Counter(self.0 + rhs.0)
  }
}

impl Add<i16> for &Counter {
  type Output = i32;

  fn add(self, rhs: i16) -> Self::Output {
    self.0 + i32::from(rhs)
  }
}

impl Add<i32> for &Counter {
  type Output = Result<i32, String>;

  fn add(self, rhs: i32) -> Self::Output {
    let sum = self.0 + rhs;
    if sum > MAX_COUNTER {
      Err("Overflow".to_string())
    } else {
      Ok(i32::try_from(sum).unwrap())
    }
  }
}

fn main() {
  assert_eq!(
    (Counter(2) + Counter(1)).0,
    3,
    "Counter + Counter -> Counter"
  );
  assert_eq!(
    (&Counter(5) + &Counter(2)).0,
    7,
    "&Counter + &Counter -> Counter"
  );
  assert_eq!(&Counter(-2) + 4i16, 2, "&Counter + i16 -> i32");
  assert_eq!(
    &Counter(1020) + 5,
    Err("Overflow".to_string()),
    "&Counter + i32 -> Result<i32, String>"
  );
}
