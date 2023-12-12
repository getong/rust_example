use std::collections::VecDeque;

fn check_remaining(input: &VecDeque<(&str, bool)>) {
  // Each item is a (&str, bool)
  for item in input {
    if item.1 == false {
      println!("You must: {}", item.0);
    }
  }
}

fn done(input: &mut VecDeque<(&str, bool)>) {
  let mut task_done = input.pop_back().unwrap(); // pop off the back
  task_done.1 = true; // now it's done - mark as true
  input.push_front(task_done); // put it at the front now
}

fn main() {
  let mut my_vec = VecDeque::from(vec![0; 600000]);
  for _i in 0..600000 {
    my_vec.pop_front(); // pop_front is like .pop but for the front
  }

  let mut my_vecdeque = VecDeque::new();
  let things_to_do = vec![
    "send email to customer",
    "add new product to list",
    "phone Loki back",
  ];

  for thing in things_to_do {
    my_vecdeque.push_front((thing, false));
  }

  done(&mut my_vecdeque);
  done(&mut my_vecdeque);

  check_remaining(&my_vecdeque);

  for task in my_vecdeque {
    print!("{:?} ", task);
  }

  let mut queue: VecDeque<String> = VecDeque::new();
  queue.push_back(String::from("first"));
  queue.push_back(String::from("second"));
  queue.push_back(String::from("third"));
  queue.push_back(String::from("fourth"));
  queue.push_front(String::from("zeroth"));
  while let Some(q_entry) = queue.pop_front() {
    println!("{}", q_entry);
  }
}
