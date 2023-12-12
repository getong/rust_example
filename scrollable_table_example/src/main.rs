use std::io;
use std::io::stdin;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tui::backend::TermionBackend;
use tui::layout::Constraint;
use tui::widgets::{Block, Row, Table, TableState};
use tui::Terminal;

use termion::event::Key;
use termion::input::MouseTerminal;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;

// enum Event<I> {
//     Input(I),
// }

pub fn next(state: &mut TableState, items: &Vec<Row>) {
  let i = match state.selected() {
    Some(i) => {
      if i >= items.len() - 1 {
        0
      } else {
        i + 1
      }
    }
    None => 0,
  };
  state.select(Some(i));
}

pub fn previous(state: &mut TableState, items: &Vec<Row>) {
  let i = match state.selected() {
    Some(i) => {
      if i == 0 {
        items.len() - 1
      } else {
        i - 1
      }
    }
    None => 0,
  };
  state.select(Some(i));
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
  let stdout = io::stdout().into_raw_mode()?;
  let stdout = MouseTerminal::from(stdout);
  let stdout = AlternateScreen::from(stdout);
  let backend = TermionBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;
  let (tx, rx) = mpsc::channel();
  let table_rows = vec![
    Row::new(vec!["row11", "row12", "row13"]),
    Row::new(vec!["row21", "row22", "row23"]),
    Row::new(vec!["row31", "row32", "row33"]),
    Row::new(vec!["row41", "row42", "row43"]),
    Row::new(vec!["row51", "row52", "row53"]),
    Row::new(vec!["row61", "row62", "row63"]),
    Row::new(vec!["row71", "row72", "row73"]),
    Row::new(vec!["row81", "row82", "row83"]),
    Row::new(vec!["row91", "row92", "row93"]),
    Row::new(vec!["row101", "row102", "row103"]),
    Row::new(vec!["row111", "row112", "row113"]),
    Row::new(vec!["row121", "row122", "row123"]),
    Row::new(vec!["row131", "row132", "row133"]),
    Row::new(vec!["row141", "row142", "row143"]),
    Row::new(vec!["row151", "row152", "row153"]),
    Row::new(vec!["row161", "row162", "row163"]),
    Row::new(vec!["row171", "row172", "row173"]),
    Row::new(vec!["row181", "row182", "row183"]),
    Row::new(vec!["row191", "row192", "row193"]),
    Row::new(vec!["row201", "row202", "row203"]),
    Row::new(vec!["row211", "row212", "row213"]),
    Row::new(vec!["row221", "row222", "row223"]),
    Row::new(vec!["row231", "row232", "row233"]),
    Row::new(vec!["row241", "row242", "row243"]),
    Row::new(vec!["row251", "row252", "row253"]),
    Row::new(vec!["row261", "row262", "row263"]),
    Row::new(vec!["row271", "row272", "row273"]),
    Row::new(vec!["row281", "row282", "row283"]),
    Row::new(vec!["row291", "row292", "row293"]),
    Row::new(vec!["row301", "row302", "row303"]),
  ];
  let mut table_state = TableState::default();
  table_state.select(Some(0));

  thread::spawn(move || loop {
    let stdin = stdin();
    for evt in stdin.keys() {
      if let Ok(key) = evt {
        if tx.send(key).is_err() {
          return;
        }
      }
    }
    thread::sleep(Duration::from_secs(3));
  });
  loop {
    terminal.draw(|f| {
      let size = f.size();
      let table = Table::new(table_rows.clone())
        .header(
          Row::new(vec!["Header1", "Header2", "Header3"]).style(
            tui::style::Style::default()
              .fg(tui::style::Color::Red)
              .add_modifier(tui::style::Modifier::BOLD),
          ),
        )
        .block(Block::default().title("Scrollable Table"))
        .widths(&[
          Constraint::Percentage(33),
          Constraint::Percentage(33),
          Constraint::Percentage(33),
        ])
        .column_spacing(1)
        .highlight_style(
          tui::style::Style::default()
            .fg(tui::style::Color::Magenta)
            .add_modifier(tui::style::Modifier::BOLD),
        )
        .highlight_symbol(">>");
      f.render_stateful_widget(table, size, &mut table_state);
    })?;
    let input = rx.recv()?;
    match input {
      Key::Char('q') => {
        break;
      }
      Key::Down => {
        next(&mut table_state, &table_rows);
      }
      Key::Up => {
        previous(&mut table_state, &table_rows);
      }
      _ => {}
    }
  }

  Ok(())
}

// copy from [Getting Started with TUI in Rust: Scrollable Table](https://medium.com/@seifelshabshiri/getting-started-with-tui-in-rust-scrollable-table-945cd50c5ca?source=tag_page---------0-84--------------------85eb3eff_2a76_4238_a97b_82c505776749-------17)
