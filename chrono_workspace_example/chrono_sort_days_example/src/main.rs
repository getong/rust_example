use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use std::{collections::HashMap, str::FromStr};

fn main() {
  // Sample data: list of (NaiveDateTime, BigDecimal) tuples
  let data = vec![
    (
      NaiveDateTime::parse_from_str("2023-10-01 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
      BigDecimal::from_str("10.5").unwrap(),
    ),
    (
      NaiveDateTime::parse_from_str("2023-10-01 15:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
      BigDecimal::from_str("5.5").unwrap(),
    ),
    (
      NaiveDateTime::parse_from_str("2023-10-02 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
      BigDecimal::from_str("20.0").unwrap(),
    ),
    (
      NaiveDateTime::parse_from_str("2023-10-02 18:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
      BigDecimal::from_str("15.0").unwrap(),
    ),
  ];

  // HashMap to store the sum of values by day
  let mut sums_by_day: HashMap<NaiveDate, BigDecimal> = HashMap::new();

  // Aggregate sums by day
  for (datetime, value) in data {
    let date = datetime.date();
    let entry = sums_by_day.entry(date).or_insert(BigDecimal::from(0));
    *entry += value;
  }

  // Convert HashMap to a Vec and sort by day
  let mut sorted_sums: Vec<(NaiveDate, BigDecimal)> = sums_by_day.into_iter().collect();
  sorted_sums.sort_by_key(|&(date, _)| date);

  println!("sorted_sums is {:#?}", sorted_sums);

  // Extract the values into a vector
  let vec_list: Vec<BigDecimal> = sorted_sums
    .iter()
    .map(|&(_, ref value)| value.clone())
    .collect();

  println!("vec_list is {:#?}", vec_list);
}
