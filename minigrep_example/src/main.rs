use minigrep_example::GrepOpts;
use std::env;
use std::process;

fn main() {
  let argv: Vec<String> = env::args().collect();
  let opts = match GrepOpts::from(&argv[1..]) {
    Ok(opts) => opts,
    Err(msg) => return eprintln!("Error: {}", msg),
  };

  match minigrep_example::run(opts) {
    Ok(found) => process::exit(!found as i32),
    Err(error) => eprintln!("Error: {}", error),
  }
}
