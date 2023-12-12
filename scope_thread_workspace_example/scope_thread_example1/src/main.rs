use std::collections::HashMap;

fn main() -> Result<(), String> {
  let numbers = vec![1, 2, 3, 4, 5, 6];

  // This creates the scope for the threads
  let (median, mode) = std::thread::scope(|scope| {
    // This scoped thread calculates average
    let median_thread = scope.spawn(|| numbers.iter().sum::<i32>() as f32 / numbers.len() as f32);

    // This scoped thread calculates mode
    let mode_thread = scope.spawn(|| {
      let mut counter = HashMap::new();
      for value in numbers.iter() {
        *counter.entry(value).or_insert(0) += 1;
      }

      counter
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(val, _)| val)
    });

    (median_thread.join(), mode_thread.join())
  });

  let (median, mode) = match (median, mode) {
    (Ok(median), Ok(Some(mode))) => Ok((median, mode)),
    _ => Err("Calculations have failed".to_string()),
  }?;

  println!("For the vector {numbers:?} the median is {median} and the mode is {mode}");
  Ok(())
}
