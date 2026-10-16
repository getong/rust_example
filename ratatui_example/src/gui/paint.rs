use eframe::egui::{self, Color32};
use ratatui::{
  buffer::Buffer,
  style::{Color, Modifier},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn grid(
  painter: &egui::Painter,
  rect: egui::Rect,
  buffer: &Buffer,
  cell_size: egui::Vec2,
  font_size: f32,
) {
  painter.rect_filled(rect, 0.0, Color32::from_rgb(17, 24, 39));
  // Background pass first: a continuation cell must not cover half a CJK glyph.
  for y in 0 .. buffer.area.height {
    for x in 0 .. buffer.area.width {
      let cell = &buffer[(x, y)];
      let (_, bg) = colors(cell.fg, cell.bg, cell.modifier);
      let pos = rect.min + egui::vec2(f32::from(x) * cell_size.x, f32::from(y) * cell_size.y);
      painter.rect_filled(egui::Rect::from_min_size(pos, cell_size), 0.0, bg);
    }
  }
  for y in 0 .. buffer.area.height {
    let mut x = 0;
    while x < buffer.area.width {
      let cell = &buffer[(x, y)];
      let width = cell
        .symbol()
        .width()
        .max(1)
        .min(usize::from(buffer.area.width - x)) as u16;
      let (fg, _) = colors(cell.fg, cell.bg, cell.modifier);
      let pos = rect.min + egui::vec2(f32::from(x) * cell_size.x, f32::from(y) * cell_size.y);
      let glyph_rect =
        egui::Rect::from_min_size(pos, egui::vec2(cell_size.x * f32::from(width), cell_size.y));
      if !cell.modifier.contains(Modifier::HIDDEN) && cell.symbol() != " " {
        let clip = painter.with_clip_rect(glyph_rect.intersect(rect));
        if border(&clip, glyph_rect, cell.symbol(), fg) {
          x += width;
          continue;
        }
        clip.text(
          glyph_rect.center(),
          egui::Align2::CENTER_CENTER,
          cell.symbol(),
          egui::FontId::monospace(font_size),
          fg,
        );
        if cell.modifier.contains(Modifier::BOLD) {
          clip.text(
            glyph_rect.center() + egui::vec2(0.5, 0.0),
            egui::Align2::CENTER_CENTER,
            cell.symbol(),
            egui::FontId::monospace(font_size),
            fg,
          );
        }
        if cell.modifier.contains(Modifier::UNDERLINED) {
          clip.line_segment(
            [
              glyph_rect.left_bottom() - egui::vec2(0.0, 3.0),
              glyph_rect.right_bottom() - egui::vec2(0.0, 3.0),
            ],
            (1.0, fg),
          );
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
          clip.line_segment(
            [glyph_rect.left_center(), glyph_rect.right_center()],
            (1.0, fg),
          );
        }
      }
      x += width;
    }
  }
}

// Draw the single-line box characters as joined strokes. Font glyphs would
// leave gaps when the logical cell size is larger than their natural advance.
fn border(painter: &egui::Painter, rect: egui::Rect, symbol: &str, color: Color32) -> bool {
  let [left, right, top, bottom] = match symbol {
    "─" => [true, true, false, false],
    "│" => [false, false, true, true],
    "┌" => [false, true, false, true],
    "┐" => [true, false, false, true],
    "└" => [false, true, true, false],
    "┘" => [true, false, true, false],
    "├" => [false, true, true, true],
    "┤" => [true, false, true, true],
    "┬" => [true, true, false, true],
    "┴" => [true, true, true, false],
    "┼" => [true, true, true, true],
    _ => return false,
  };
  for (enabled, end) in [
    (left, rect.left_center()),
    (right, rect.right_center()),
    (top, rect.center_top()),
    (bottom, rect.center_bottom()),
  ] {
    if enabled {
      painter.line_segment([rect.center(), end], (1.0, color));
    }
  }
  true
}

fn colors(fg: Color, bg: Color, modifier: Modifier) -> (Color32, Color32) {
  let mut fg = color(fg, Color32::LIGHT_GRAY);
  let mut bg = color(bg, Color32::from_rgb(17, 24, 39));
  if modifier.contains(Modifier::REVERSED) {
    std::mem::swap(&mut fg, &mut bg);
  }
  if modifier.contains(Modifier::DIM) {
    fg = fg.gamma_multiply(0.6);
  }
  (fg, bg)
}

fn color(value: Color, fallback: Color32) -> Color32 {
  let index = match value {
    Color::Reset => return fallback,
    Color::Rgb(r, g, b) => return Color32::from_rgb(r, g, b),
    Color::Indexed(index) => index,
    Color::Black => 0,
    Color::Red => 1,
    Color::Green => 2,
    Color::Yellow => 3,
    Color::Blue => 4,
    Color::Magenta => 5,
    Color::Cyan => 6,
    Color::Gray => 7,
    Color::DarkGray => 8,
    Color::LightRed => 9,
    Color::LightGreen => 10,
    Color::LightYellow => 11,
    Color::LightBlue => 12,
    Color::LightMagenta => 13,
    Color::LightCyan => 14,
    Color::White => 15,
  };
  const ANSI: [[u8; 3]; 16] = [
    [0, 0, 0],
    [205, 49, 49],
    [13, 188, 121],
    [229, 229, 16],
    [36, 114, 200],
    [188, 63, 188],
    [17, 168, 205],
    [204, 204, 204],
    [102, 102, 102],
    [241, 76, 76],
    [35, 209, 139],
    [245, 245, 67],
    [59, 142, 234],
    [214, 112, 214],
    [41, 184, 219],
    [255, 255, 255],
  ];
  let [r, g, b] = match index {
    0 ..= 15 => ANSI[usize::from(index)],
    16 ..= 231 => {
      let n = index - 16;
      let levels = [0, 95, 135, 175, 215, 255];
      [
        levels[usize::from(n / 36)],
        levels[usize::from(n / 6 % 6)],
        levels[usize::from(n % 6)],
      ]
    }
    _ => [8 + (index - 232) * 10; 3],
  };
  Color32::from_rgb(r, g, b)
}
