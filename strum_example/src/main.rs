use std::str::FromStr;

use strum::{EnumCount, EnumMessage, EnumProperty, IntoEnumIterator, VariantNames};
use strum_macros::{
  AsRefStr, Display, EnumCount, EnumDiscriminants, EnumIter, EnumMessage, EnumProperty, EnumString,
  IntoStaticStr, VariantNames,
};

fn main() -> Result<(), strum::ParseError> {
  println!("strum turns enums into small, type-safe lookup tables.");
  println!();

  show_mode_parsing()?;
  show_status_iteration();
  show_command_metadata();
  show_discriminants();

  Ok(())
}

fn show_mode_parsing() -> Result<(), strum::ParseError> {
  // EnumString parses text into enum variants; Display formats variants back.
  let mode = Mode::from_str("safe")?;

  println!("1. Parse user input into an enum:");
  println!("   input \"safe\" -> {mode:?}");
  println!("   display name -> {mode}");
  println!("   stable str -> {}", mode.as_ref());
  println!("   is_fast? {}", mode.is_fast());
  println!();

  Ok(())
}

fn show_status_iteration() {
  // EnumIter and EnumCount let code discover every variant without a manual list.
  println!("2. Iterate over all status variants:");
  println!("   count -> {}", Status::COUNT);

  for status in Status::iter() {
    let label: &'static str = status.into();
    println!(
      "   {status:<12} css={}",
      status.get_str("css").unwrap_or("unknown")
    );
    println!("      stable label: {label}");
  }

  println!();
}

fn show_command_metadata() {
  // VariantNames exposes a static list, and EnumMessage keeps help text with variants.
  println!("3. Build CLI/help data from enum metadata:");
  println!("   commands -> {}", Command::VARIANTS.join(", "));

  for command in Command::iter() {
    println!(
      "   {command:<8} {}",
      command.get_message().unwrap_or("no description"),
    );
  }

  println!();
}

fn show_discriminants() {
  // EnumDiscriminants creates a payload-free enum for matching by variant kind.
  let events = [
    Event::Connected { peer: "node-a" },
    Event::Message { bytes: 512 },
    Event::Disconnected,
  ];

  println!("4. Match payload variants by discriminant:");

  for event in events {
    let kind = EventDiscriminants::from(&event);
    match &event {
      Event::Connected { peer } => println!("   peer {peer} -> {kind:?}"),
      Event::Message { bytes } => println!("   message with {bytes} bytes -> {kind:?}"),
      Event::Disconnected => println!("   disconnected -> {kind:?}"),
    }
  }
}

#[derive(
  Debug, PartialEq, Eq, EnumString, Display, AsRefStr, IntoStaticStr, strum_macros::EnumIs,
)]
#[strum(serialize_all = "kebab-case", ascii_case_insensitive)]
enum Mode {
  Fast,
  Safe,
  DebugTrace,
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumCount, Display, IntoStaticStr, EnumProperty,
)]
#[strum(serialize_all = "snake_case")]
enum Status {
  #[strum(props(css = "is-idle"))]
  Idle,
  #[strum(props(css = "is-running"))]
  Running,
  #[strum(props(css = "is-failed"))]
  Failed,
}

#[derive(Debug, EnumIter, Display, VariantNames, EnumMessage)]
#[strum(serialize_all = "kebab-case")]
enum Command {
  #[strum(message = "Create a new project")]
  Init,
  #[strum(message = "Compile the current project")]
  Build,
  #[strum(message = "Run the project test suite")]
  Test,
}

#[derive(Debug, EnumDiscriminants)]
enum Event {
  Connected { peer: &'static str },
  Message { bytes: usize },
  Disconnected,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_case_insensitive_kebab_case_modes() {
    assert_eq!(Mode::from_str("debug-trace").ok(), Some(Mode::DebugTrace));
    assert_eq!(Mode::from_str("SAFE").ok(), Some(Mode::Safe));
  }

  #[test]
  fn iterates_every_status_variant() {
    let statuses = Status::iter().collect::<Vec<_>>();

    assert_eq!(statuses.len(), Status::COUNT);
    assert!(statuses.contains(&Status::Running));
  }

  #[test]
  fn exposes_variant_names_and_messages() {
    assert_eq!(Command::VARIANTS, ["init", "build", "test"]);

    let messages = Command::iter()
      .filter_map(|command| command.get_message())
      .collect::<Vec<_>>();

    assert!(messages.contains(&"Compile the current project"));
  }
}
