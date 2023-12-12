use clap::{Arg, Command};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
  let matches = Command::new("Async CLI")
    .version("1.0")
    .author("Your Name")
    .about("A simple CLI tool with async functionality")
    .subcommand(Command::new("sync").about("Performs a synchronous operation"))
    .subcommand(
      Command::new("async")
        .about("Performs an asynchronous operation using tokio::spawn")
        .arg(
          Arg::new("delay")
            .short('d')
            .long("delay")
            .required(true)
            .default_value("1000")
            .help("Sets the delay in milliseconds for the async operation"),
        ),
    )
    .get_matches();

  if let Some(_) = matches.subcommand_matches("sync") {
    println!("Performing synchronous operation...");
    // Perform synchronous operation here
    println!("Synchronous operation completed.");
  } else if let Some(matches) = matches.subcommand_matches("async") {
    let delay_ms = matches
      .get_one::<String>("delay")
      // .value_of("delay")
      .unwrap_or(&"1000".to_string())
      .parse::<u64>()
      .unwrap();
    println!(
      "Performing asynchronous operation with {} ms delay...",
      delay_ms
    );

    // Using tokio::spawn to run an asynchronous task
    tokio::spawn(async move {
      sleep(Duration::from_millis(delay_ms)).await;
      println!("Async operation completed after {} ms.", delay_ms);
    });

    // Need to sleep here to keep the main thread alive while the async task runs
    sleep(Duration::from_secs(2)).await;
  } else {
    println!("No subcommand specified. Use --help for usage information.");
  }
}
