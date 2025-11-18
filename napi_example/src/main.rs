use napi_example::{
  Greeter, add, average, call_typescript_transformer_example, emit_typescript_events_example,
  orchestrate_typescript_decision_example, repeat, run_typescript_callback_example, summarize,
  version,
};

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

  match run_typescript_callback_example("TypeScript callbacks".to_string()) {
    Ok(response) => println!("run_typescript_callback_example -> {}", response),
    Err(err) => eprintln!("run_typescript_callback_example error: {}", err),
  }

  match emit_typescript_events_example(vec![
    "alpha".to_string(),
    "beta".to_string(),
    "gamma".to_string(),
  ]) {
    Ok(events) => {
      println!("emit_typescript_events_example collected:");
      for event in events {
        println!("  {}", event);
      }
    }
    Err(err) => eprintln!("emit_typescript_events_example error: {}", err),
  }

  match call_typescript_transformer_example("Rust <-> TypeScript".to_string(), 3) {
    Ok(result) => println!("call_typescript_transformer_example -> {}", result),
    Err(err) => eprintln!("call_typescript_transformer_example error: {}", err),
  }

  match orchestrate_typescript_decision_example("Ship napi example".to_string()) {
    Ok(summary) => {
      println!(
        "orchestrate_typescript_decision_example decision: {}",
        if summary.decision {
          "approved"
        } else {
          "rejected"
        }
      );
      for entry in summary.log {
        println!("  {}", entry);
      }
    }
    Err(err) => eprintln!("orchestrate_typescript_decision_example error: {}", err),
  }
}
