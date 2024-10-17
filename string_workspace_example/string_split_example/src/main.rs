fn split_frequency(s: &str) -> Vec<(i32, i64)> {
  let mut list: Vec<(i32, i64)> = vec![];
  for v in s.split(",") {
    let mut slice = v.split(":");
    let s1 = slice.next();
    let s2 = slice.next();
    match (s1, s2) {
      (Some(ss1), Some(ss2)) => {
        let v: i32 = ss1.parse().unwrap_or(0);
        let t: i64 = ss2.parse().unwrap_or(0);
        list.push((v, t))
      }
      _ => (),
    }
  }
  list
}

fn merge_frequency(ids: &[(i32, i64)]) -> String {
  let mut list: Vec<String> = vec![];
  for (v, t) in ids {
    list.push(format!("{}:{}", v, t));
  }
  list.join(",")
}

fn main() {
  let input = "abc |||cde";
  let first_part = input.split("|||").next().unwrap_or(input).trim();
  println!("first part of {} || is {}", input, first_part);

  let input = "abc cde";
  let first_part = input.split("|||").next().unwrap().trim();
  println!("first part of {} || is {}", input, first_part);

  let frequency_list = [(1, 1_i64), (2, 2_i64), (3, 3_i64)];
  let frequency_string = merge_frequency(&frequency_list);
  println!("frequency_string:  {}", frequency_string);
  let split_frequency_list = split_frequency(&frequency_string);
  println!("split_frequency: {:?}", split_frequency_list);

  println!();

  let frequency_list = [(1, 1_i64)];
  let frequency_string = merge_frequency(&frequency_list);
  println!("frequency_string:  {}", frequency_string);
  let split_frequency_list = split_frequency(&frequency_string);
  println!("split_frequency: {:?}", split_frequency_list);
}

// first part of abc |||cde || is abc
// first part of abc cde || is abc cde
// frequency_string:  1:1,2:2,3:3
// split_frequency: [(1, 1), (2, 2), (3, 3)]

// frequency_string:  1:1
// split_frequency: [(1, 1)]
