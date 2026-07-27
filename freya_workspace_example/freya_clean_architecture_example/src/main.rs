use freya::prelude::{LaunchConfig, WindowConfig, launch};
use freya_clean_architecture_example::app;

fn main() {
  launch(LaunchConfig::new().with_window(WindowConfig::new(app)))
}
