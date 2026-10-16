//! A persistent character grid at the same Backend boundary as Obelus.
//! Ratatui sends diffs, so covered cells of wide glyphs must be cleared here.
use std::io;

use ratatui::{
  backend::{Backend, ClearType, WindowSize},
  buffer::{Buffer, Cell},
  layout::{Position, Rect, Size},
};
use unicode_width::UnicodeWidthStr;

pub(super) struct Grid {
  pub buffer: Buffer,
  pub cursor: Option<Position>,
  position: Position,
}

impl Grid {
  pub fn new(width: u16, height: u16) -> Self {
    Self {
      buffer: Buffer::empty(Rect::new(0, 0, width, height)),
      cursor: None,
      position: Position::ORIGIN,
    }
  }

  pub fn resize(&mut self, width: u16, height: u16) {
    self.buffer = Buffer::empty(Rect::new(0, 0, width, height));
  }
}

impl Backend for Grid {
  type Error = io::Error;

  fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
  where
    I: Iterator<Item = (u16, u16, &'a Cell)>,
  {
    for (x, y, cell) in content {
      if !self.buffer.area.contains(Position::new(x, y)) {
        continue;
      }
      self.buffer[(x, y)] = cell.clone();
      let width = cell.symbol().width().max(1).min(usize::from(u16::MAX)) as u16;
      for covered in x.saturating_add(1) .. x.saturating_add(width).min(self.buffer.area.width) {
        let mut blank = cell.clone();
        blank.set_symbol(" ");
        self.buffer[(covered, y)] = blank;
      }
    }
    Ok(())
  }

  fn hide_cursor(&mut self) -> io::Result<()> {
    self.cursor = None;
    Ok(())
  }
  fn show_cursor(&mut self) -> io::Result<()> {
    self.cursor = Some(self.position);
    Ok(())
  }
  fn get_cursor_position(&mut self) -> io::Result<Position> {
    Ok(self.position)
  }
  fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
    self.position = position.into();
    if self.cursor.is_some() {
      self.cursor = Some(self.position);
    }
    Ok(())
  }
  fn clear(&mut self) -> io::Result<()> {
    self.buffer.reset();
    Ok(())
  }
  fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
    let width = usize::from(self.buffer.area.width);
    let cursor = usize::from(self.position.y) * width + usize::from(self.position.x);
    for (index, cell) in self.buffer.content.iter_mut().enumerate() {
      let clear = match clear_type {
        ClearType::All => true,
        ClearType::AfterCursor => index > cursor,
        ClearType::BeforeCursor => index < cursor,
        ClearType::CurrentLine => width > 0 && index / width == usize::from(self.position.y),
        ClearType::UntilNewLine => {
          width > 0 && index >= cursor && index / width == usize::from(self.position.y)
        }
      };
      if clear {
        cell.reset();
      }
    }
    Ok(())
  }
  fn size(&self) -> io::Result<Size> {
    Ok(self.buffer.area.as_size())
  }
  fn window_size(&mut self) -> io::Result<WindowSize> {
    Ok(WindowSize {
      columns_rows: self.size()?,
      pixels: Size::default(),
    })
  }
  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, style::Color, widgets::Paragraph};

  use super::*;

  #[test]
  fn wide_glyph_replaces_stale_cell_and_can_be_replaced_by_ascii() {
    let mut terminal = Terminal::new(Grid::new(8, 2)).unwrap();
    terminal
      .draw(|f| f.render_widget(Paragraph::new("abcd"), f.area()))
      .unwrap();
    terminal
      .draw(|f| f.render_widget(Paragraph::new("中文").style(Color::Yellow), f.area()))
      .unwrap();
    let grid = terminal.backend();
    assert_eq!(grid.buffer[(0, 0)].symbol(), "中");
    assert_eq!(grid.buffer[(1, 0)].symbol(), " ");
    assert_eq!(grid.buffer[(1, 0)].fg, Color::Yellow);
    assert_eq!(grid.buffer[(3, 0)].symbol(), " ");
    terminal
      .draw(|f| f.render_widget(Paragraph::new("x"), f.area()))
      .unwrap();
    assert_eq!(terminal.backend().buffer[(0, 0)].symbol(), "x");
    assert_eq!(terminal.backend().buffer[(2, 0)].symbol(), " ");
  }
}
