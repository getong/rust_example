use enum_map::{enum_map, Enum};

#[derive(Debug, Enum)]
enum Example {
  A,
  B,
  C,
}

fn main() {
  let mut map = enum_map! {
    Example::A => "a".to_string(),
    Example::B => "b".to_string(),
    Example::C => "c".to_string(),
  };
  map[Example::C] = "d".to_string();

  assert_eq!(map[Example::A], "a".to_string());

  for (key, &ref value) in &map {
    println!("{:?} has {} as value.", key, value);
  }
}
