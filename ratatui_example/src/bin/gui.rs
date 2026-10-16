#![cfg_attr(not(test), windows_subsystem = "windows")]

fn main() -> eframe::Result {
  ratatui_example::gui::run()
}
