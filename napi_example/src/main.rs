use napi_example::{Greeter, add, average, repeat, summarize, version};

fn main() {
  println!("napi example version: {}", version());
  println!("add(40, 2) = {}", add(40, 2));
  println!("repeat('na', 4) = {}", repeat("na".to_string(), 4));

  match average(vec![1.0, 2.0, 3.0, 4.0]) {
    Ok(avg) => println!("average([1,2,3,4]) = {:.2}", avg),
    Err(err) => eprintln!("average error: {}", err),
  }

  match summarize(vec![3, 1, 4, 1, 5, 9]) {
    Ok(summary) => {
      println!("summarize input = {:?}", summary.input);
      println!("summarize sum = {}", summary.sum);
      println!("summarize calculated_at = {}", summary.calculated_at);
    }
    Err(err) => eprintln!("summarize error: {}", err),
  }

  let mut greeter = Greeter::new(Some("Hello from napi".to_string()));
  println!("{}", greeter.greet("world".to_string()));

  let eager = greeter.greet_many(vec!["Alice".into(), "Bob".into()]);
  println!("greet_many = {:?}", eager);

  greeter.set_greeting("Hola".to_string());
  println!("{}", greeter.greet("Carlos".to_string()));
}
