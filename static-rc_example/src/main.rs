use static_rc::StaticRc;

type Full<T> = StaticRc<T, 3, 3>;
// type TwoThird<T> = StaticRc<T, 2, 3>;
// type OneThird<T> = StaticRc<T, 1, 3>;

fn main() {
  // println!("Hello, world!");

  let mut full = Full::new("Hello, world!".to_string());

  assert_eq!("Hello, world!", &*full);

  //  Mutation is allowed when having full ownership, just like for `Box`.
  *full = "Hello, you!".to_string();

  assert_eq!("Hello, you!", &*full);

  //  Mutation is no longer allowed from now on, due to aliasing, just like for `Rc`.
  let (two_third, one_third) = Full::split::<2, 1>(full);

  assert_eq!("Hello, you!", &*two_third);
  assert_eq!("Hello, you!", &*one_third);

  let mut full = Full::join(one_third, two_third);

  assert_eq!("Hello, you!", &*full);

  //  Mutation is allowed again, since `full` has full ownership.
  *full = "Hello, world!".to_string();

  assert_eq!("Hello, world!", &*full);

  //  Finally, the value is dropped when `full` is.
}
