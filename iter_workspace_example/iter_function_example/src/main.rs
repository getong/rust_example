use std::iter;

fn fibonacci() -> impl Iterator<Item = usize> {
  let mut state = (0, 1);
  iter::from_fn(move || {
    state = (state.1, state.0 + state.1);
    Some(state.0)
  })
}

fn skip_while() {
  let arr = [
    10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
  ];
  let collection: Vec<_> = arr
    .iter()
    .cloned()
    .skip_while(|&elm| elm < 25)
    .take_while(|&elm| elm <= 100)
    .collect();
  for elm in collection {
    println!("skip while elm: {}", elm);
  }
}

fn filter() {
  let arr = [
    10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
  ];
  let collection: Vec<_> = arr
    .iter()
    .enumerate()
    .filter(|&(i, _)| i % 2 != 0)
    .map(|(_, elm)| elm)
    .collect();
  for elm in collection {
    println!("filter function elm: {}", elm);
  }
}

fn filter_map() {
  let arr = [
    10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
  ];
  let collection: Vec<_> = arr
    .iter()
    .enumerate()
    .filter_map(|(i, elm)| if i % 2 != 0 { Some(elm) } else { None })
    .collect();
  for elm in collection {
    println!("filter_map elm: {}", elm);
  }
}

fn filter_map2() {
  let a = vec![
    "1",
    "2",
    "-1",
    "4",
    "-4",
    "100",
    "invalid",
    "Not a number",
    "",
  ];
  let only_positive_numbers: Vec<i64> = a
    .iter()
    .filter_map(|&x| x.parse::<i64>().ok())
    .filter(|&x| x > 0)
    .collect();

  println!(
    "filter_map2, only_positive_numbers: {:?}",
    only_positive_numbers
  );
}

fn fold() {
  let arr = [
    10u32, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
  ];
  let sum = arr.iter().fold(0u32, |acc, elm| acc + elm);
  println!("fold sum: {}", sum);
}

fn zip() {
  let arr1 = [
    10u32, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
  ];
  let arr2 = [
    25u32, 12, 73, 2, 98, 122, 213, 22, 39, 300, 144, 163, 127, 3, 56,
  ];
  let collection: Vec<_> = arr1
    .iter()
    .zip(arr2.iter())
    .map(|(elm1, elm2)| elm1 + elm2)
    .collect();
  println!("zip collection: {:?}", collection);
}

fn for_each() {
  let num_vec = vec![10, 9, 8];

  num_vec
    .iter() // iter over num_vec
    .enumerate() // get (index, number)
    .for_each(|(index, number)| println!("Index number {} has number {}", index, number));
  // do something for each one
}

fn main() {
  // println!("Hello, world!");
  assert_eq!(
    fibonacci().take(8).collect::<Vec<_>>(),
    vec![1, 1, 2, 3, 5, 8, 13, 21]
  );

  let powers_of_10 = iter::successors(Some(1_u16), |n| n.checked_mul(10));
  assert_eq!(
    powers_of_10.collect::<Vec<_>>(),
    &[1, 10, 100, 1_000, 10_000]
  );

  skip_while();

  println!();

  filter();

  println!();

  filter_map();
  filter_map2();

  println!();
  fold();

  println!();
  zip();

  println!();
  for_each();
}
