use std::{env, process};

use minigrep_example::GrepOpts;

fn main() {
  let argv: Vec<String> = env::args().collect();
  let opts = match GrepOpts::from(&argv[1 ..]) {
    Ok(opts) => opts,
    Err(msg) => return eprintln!("Error: {}", msg),
  };

  match minigrep_example::run(opts) {
    Ok(found) => process::exit(!found as i32),
    Err(error) => eprintln!("Error: {}", error),
  }
}
