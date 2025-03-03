use std::{error::Error, thread, time::Duration};

use signal_hook::{consts::SIGINT, iterator::Signals};

fn main() -> Result<(), Box<dyn Error>> {
  let mut signals = Signals::new(&[SIGINT])?;

  thread::spawn(move || {
    for sig in signals.forever() {
      println!("Received signal {:?}", sig);
    }
  });

  // Following code does the actual work, and can be interrupted by pressing
  // Ctrl-C. As an example: Let's wait a few seconds.
  thread::sleep(Duration::from_secs(2));

  Ok(())
}
