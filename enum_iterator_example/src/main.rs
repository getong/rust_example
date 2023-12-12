use enum_iterator::{all, cardinality, first, last, next, previous, reverse_all, Sequence};

#[derive(Debug, PartialEq, Sequence)]
enum Day {
  Monday,
  Tuesday,
  Wednesday,
  Thursday,
  Friday,
  Saturday,
  Sunday,
}

#[derive(Debug, PartialEq, Sequence)]
struct Foo {
  a: bool,
  b: u8,
}

fn main() {
  // println!("Hello, world!");
  assert_eq!(cardinality::<Day>(), 7);

  assert_eq!(
    all::<Day>().collect::<Vec<_>>(),
    [
      Day::Monday,
      Day::Tuesday,
      Day::Wednesday,
      Day::Thursday,
      Day::Friday,
      Day::Saturday,
      Day::Sunday,
    ]
  );

  assert_eq!(first::<Day>(), Some(Day::Monday));

  assert_eq!(last::<Day>(), Some(Day::Sunday));
  assert_eq!(next(&Day::Tuesday), Some(Day::Wednesday));
  assert_eq!(previous(&Day::Wednesday), Some(Day::Tuesday));
  assert_eq!(
    reverse_all::<Day>().collect::<Vec<_>>(),
    [
      Day::Sunday,
      Day::Saturday,
      Day::Friday,
      Day::Thursday,
      Day::Wednesday,
      Day::Tuesday,
      Day::Monday,
    ]
  );

  assert_eq!(cardinality::<Foo>(), 512);
  assert_eq!(first::<Foo>(), Some(Foo { a: false, b: 0 }));
  assert_eq!(last::<Foo>(), Some(Foo { a: true, b: 255 }));
}
