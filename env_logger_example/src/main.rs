use log::{error, info, warn};

fn main() {
  // Initialize env_logger to configure log level based on environment variables
  env_logger::init();

  info!("This is an info message");
  warn!("This is a warning message");
  error!("This is an error message");
}

// copy from https://dev.to/trish_07/top-5-rust-crates-to-make-development-easier-gi8