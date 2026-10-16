use crossterm::event::{
  Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use eframe::egui;

fn modifiers(value: egui::Modifiers) -> KeyModifiers {
  let mut result = KeyModifiers::NONE;
  if value.shift {
    result |= KeyModifiers::SHIFT;
  }
  if value.ctrl {
    result |= KeyModifiers::CONTROL;
  }
  if value.alt {
    result |= KeyModifiers::ALT;
  }
  if value.mac_cmd {
    result |= KeyModifiers::SUPER;
  }
  result
}

fn position(
  pos: egui::Pos2,
  rect: egui::Rect,
  cell: egui::Vec2,
  columns: u16,
  rows: u16,
) -> Option<(u16, u16)> {
  if !rect.contains(pos) {
    return None;
  }
  let x = ((pos.x - rect.left()) / cell.x).floor() as u16;
  let y = ((pos.y - rect.top()) / cell.y).floor() as u16;
  (x < columns && y < rows).then_some((x, y))
}

pub(super) fn translate(
  event: &egui::Event,
  rect: egui::Rect,
  cell: egui::Vec2,
  columns: u16,
  rows: u16,
) -> Vec<Event> {
  let mouse = |point: Option<(u16, u16)>, kind, mods| {
    let (column, row) = point.unwrap_or((u16::MAX, u16::MAX));
    Event::Mouse(MouseEvent {
      kind,
      column,
      row,
      modifiers: mods,
    })
  };
  let key = |code, mods| Event::Key(KeyEvent::new(code, mods));
  match event {
    // Printable characters arrive via Text, never again via Key.
    egui::Event::Text(text) => text
      .chars()
      .filter(|ch| !ch.is_control() && *ch != ' ')
      .map(|ch| key(KeyCode::Char(ch), KeyModifiers::NONE))
      .collect(),
    egui::Event::Key {
      key: value,
      pressed: true,
      modifiers: mods,
      repeat: false,
      ..
    } => {
      let code = match value {
        egui::Key::Tab if mods.shift => KeyCode::BackTab,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Space => KeyCode::Char(' '),
        egui::Key::Escape => KeyCode::Esc,
        egui::Key::ArrowLeft => KeyCode::Left,
        egui::Key::ArrowRight => KeyCode::Right,
        egui::Key::ArrowUp => KeyCode::Up,
        egui::Key::ArrowDown => KeyCode::Down,
        egui::Key::C if mods.ctrl => KeyCode::Char('c'),
        _ => return vec![],
      };
      vec![key(code, modifiers(*mods))]
    }
    // Egui consumes Ctrl+C into Copy on some platforms.
    egui::Event::Copy => vec![key(KeyCode::Char('c'), KeyModifiers::CONTROL)],
    egui::Event::PointerMoved(pos) => vec![mouse(
      position(*pos, rect, cell, columns, rows),
      MouseEventKind::Moved,
      KeyModifiers::NONE,
    )],
    egui::Event::PointerGone => vec![mouse(None, MouseEventKind::Moved, KeyModifiers::NONE)],
    egui::Event::PointerButton {
      pos,
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers: mods,
    } => position(*pos, rect, cell, columns, rows)
      .map(|point| {
        vec![mouse(
          Some(point),
          MouseEventKind::Down(MouseButton::Left),
          modifiers(*mods),
        )]
      })
      .unwrap_or_default(),
    _ => vec![],
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn translate_event(event: egui::Event) -> Vec<Event> {
    translate(
      &event,
      egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(110.0, 260.0)),
      egui::vec2(11.0, 26.0),
      10,
      10,
    )
  }

  #[test]
  fn pointer_uses_grid_coordinates_and_leaving_clears_hover() {
    let events = translate_event(egui::Event::PointerMoved(egui::pos2(33.0, 74.0)));
    assert!(matches!(
      events[0],
      Event::Mouse(MouseEvent {
        column: 2,
        row: 2,
        ..
      })
    ));
    let events = translate_event(egui::Event::PointerGone);
    assert!(matches!(
      events[0],
      Event::Mouse(MouseEvent {
        column: u16::MAX,
        ..
      })
    ));
  }

  #[test]
  fn text_and_key_do_not_activate_space_twice() {
    assert!(translate_event(egui::Event::Text(" ".into())).is_empty());
    let events = translate_event(egui::Event::Key {
      key: egui::Key::Space,
      physical_key: None,
      pressed: true,
      repeat: false,
      modifiers: egui::Modifiers::NONE,
    });
    assert_eq!(events.len(), 1);
    assert_eq!(translate_event(egui::Event::Text("+".into())).len(), 1);
  }
}
