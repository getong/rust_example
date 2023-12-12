use std::collections::BTreeSet; // Change HashSet to BTreeSet

fn main() {
  let many_numbers = vec![
    94, 42, 59, 64, 32, 22, 38, 5, 59, 49, 15, 89, 74, 29, 14, 68, 82, 80, 56, 41, 36, 81, 66, 51,
    58, 34, 59, 44, 19, 93, 28, 33, 18, 46, 61, 76, 14, 87, 84, 73, 71, 29, 94, 10, 35, 20, 35, 80,
    8, 43, 79, 25, 60, 26, 11, 37, 94, 32, 90, 51, 11, 28, 76, 16, 63, 95, 13, 60, 59, 96, 95, 55,
    92, 28, 3, 17, 91, 36, 20, 24, 0, 86, 82, 58, 93, 68, 54, 80, 56, 22, 67, 82, 58, 64, 80, 16,
    61, 57, 14, 11,
  ];

  let mut number_btreeset = BTreeSet::new(); // Change HashSet to BTreeSet

  for number in many_numbers {
    number_btreeset.insert(number);
  }
  for entry in number_btreeset {
    print!("{} ", entry);
  }
  println!();

  let mut set = BTreeSet::new();
  set.insert(42);
  set.insert(1);
  set.insert(3);
  let first = set.iter().next();
  assert_eq!(first, Some(&1));
  let last = set.iter().last();
  assert_eq!(last, Some(&42));
  let last = set.iter().next_back();
  assert_eq!(last, Some(&42));
  let max = set.iter().max();
  assert_eq!(max, Some(&42));

  let set: BTreeSet<usize> = [1, 2, 3, 4].iter().cloned().collect();
  let v: Vec<_> = set.into_iter().collect();
  assert_eq!(v, &[1, 2, 3, 4]);
  // set can be no more used here
  // println!("set : {:?}", set);

  let set2: BTreeSet<usize> = [1, 2, 3, 4].iter().cloned().collect();
  let v: Vec<_> = (&set2).into_iter().cloned().collect();
  assert_eq!(v, &[1, 2, 3, 4]);
  // set2 can be used here
  println!("set2 : {:?}", set2);
}
