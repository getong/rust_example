use chrono::prelude::*;

fn main() {
  // Get the current time
  let now = Utc::now().naive_utc().and_utc().timestamp();

  // Convert the current time to seconds since the Unix epoch and divide by 60
  let now_minutes = now / 600;

  // "5:2883906,3:2883907,2008:2883908,3006:2883909,3017:2883910,4028:2883911,2009:2883912",
  // hour_frequency: 9050
  println!("now_minutes: {}", now_minutes);

  // Create a list of tuples
  let mut time_list = Vec::new();
  for i in (0 ..= 6).rev() {
    time_list.push((i, now_minutes - i as i64));
  }

  println!("All the list is:");
  // Print the list
  for (num, time) in &time_list {
    println!("({}, {})", num, time);
  }

  let the_last_five_time = now_minutes - 5;
  let mut select_index = 0;
  for (i, (_v, t)) in time_list.iter().enumerate() {
    if *t < the_last_five_time {
      select_index = i + 1;
    }
  }

  let mut total = 0;
  for (v, t) in &time_list {
    if *t >= the_last_five_time {
      total += v;
    }
  }

  println!(
    "the last five time is {}, total is {}",
    the_last_five_time, total
  );

  println!(
    "The select_index is {}, now_minutes is {}, the last five time is {}",
    select_index, now_minutes, the_last_five_time
  );
  for (num, time) in &time_list[select_index ..] {
    println!("({}, {})", num, time);
  }

  let the_last_six_time = now_minutes - 6;
  let mut select_index = 0;
  for (i, (_v, t)) in time_list.iter().enumerate() {
    if *t < the_last_six_time {
      select_index = i + 1;
    }
  }

  println!(
    "The select_index is {}, now_minutes is {}, the last six time is {}",
    select_index, now_minutes, the_last_six_time
  );
  for (num, time) in &time_list[select_index ..] {
    println!("({}, {})", num, time);
  }

  let mut total = 0;
  for (v, t) in &time_list {
    if *t >= the_last_six_time {
      total += v;
    }
  }

  println!(
    "the last six time is {}, total is {}",
    the_last_six_time, total
  );

  let the_last_six_time = now_minutes - 6;
  let mut select_index = 0;
  for (i, (_v, t)) in time_list.iter().enumerate() {
    if *t <= the_last_six_time {
      select_index = i + 1;
    }
  }

  println!(
    "The select_index is {}, now_minutes is {}, the last six time is {}",
    select_index, now_minutes, the_last_six_time
  );
  for (num, time) in &time_list[select_index ..] {
    println!("({}, {})", num, time);
  }

  let mut total = 0;
  for (v, t) in &time_list {
    if *t > the_last_six_time {
      total += v;
    }
  }

  println!(
    "the the_last_six_time is {}, total is {}",
    the_last_six_time, total
  );
}
