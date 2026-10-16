//! Window transport only: the widgets and all actions live in `app`.
mod grid;
mod input;
mod paint;

use std::{io, path::PathBuf};

use eframe::egui;
use grid::Grid;
use ratatui::{Terminal, layout::Rect};

use crate::app::App;

const CELL: egui::Vec2 = egui::vec2(11.0, 26.0);
const FONT_SIZE: f32 = 18.0;

pub fn run() -> eframe::Result {
  let options = eframe::NativeOptions {
    renderer: eframe::Renderer::Wgpu,
    viewport: egui::ViewportBuilder::default()
      .with_title("Ratatui · GPU 窗口")
      .with_inner_size([1000.0, 720.0])
      .with_min_inner_size([300.0, 200.0]),
    ..Default::default()
  };
  eframe::run_native(
    "ratatui-gui",
    options,
    Box::new(|cc| {
      install_fonts(&cc.egui_ctx)?;
      Ok(Box::new(Window {
        app: App::default(),
        terminal: Terminal::new(Grid::new(1, 1))?,
        error: None,
      }))
    }),
  )
}

struct Window {
  app: App,
  terminal: Terminal<Grid>,
  error: Option<String>,
}

impl Window {
  fn draw(&mut self) -> io::Result<()> {
    self.terminal.draw(|frame| self.app.draw(frame))?;
    Ok(())
  }

  fn update_grid(&mut self, ctx: &egui::Context, rect: egui::Rect) -> io::Result<()> {
    let columns = (rect.width() / CELL.x).floor().clamp(1.0, 1000.0) as u16;
    let rows = (rect.height() / CELL.y).floor().clamp(1.0, 1000.0) as u16;
    let size = Rect::new(0, 0, columns, rows);
    if self.terminal.backend().buffer.area != size {
      self.terminal.backend_mut().resize(columns, rows);
      self.terminal.resize(size)?;
      self
        .app
        .handle(crossterm::event::Event::Resize(columns, rows));
    }
    // Layout before input, and after each event: hit targets must reflect
    // a resize or a dialog opened earlier in this very same input batch.
    self.draw()?;
    let events = ctx.input(|input| input.events.clone());
    for event in events {
      for translated in input::translate(&event, rect, CELL, columns, rows) {
        self.app.handle(translated);
        self.draw()?;
        if self.app.quit {
          ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          return Ok(());
        }
      }
    }
    Ok(())
  }
}

impl eframe::App for Window {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    egui::CentralPanel::default()
      .frame(egui::Frame::NONE)
      .show(ui, |ui| {
        let rect = ui.max_rect();
        if let Err(error) = self.update_grid(ui.ctx(), rect) {
          self.error = Some(error.to_string());
        }
        if let Some(error) = &self.error {
          ui.colored_label(egui::Color32::RED, format!("窗口绘制失败：{error}"));
        } else {
          paint::grid(
            ui.painter(),
            rect,
            &self.terminal.backend().buffer,
            CELL,
            FONT_SIZE,
          );
        }
      });
  }
}

fn install_fonts(ctx: &egui::Context) -> io::Result<()> {
  let override_path = std::env::var_os("RATATUI_GUI_FONT").map(PathBuf::from);
  let candidates = [
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "C:/Windows/Fonts/msyh.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
  ];
  let path = override_path.or_else(|| {
    candidates
      .iter()
      .map(PathBuf::from)
      .find(|path| path.exists())
  });
  let path = path.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::NotFound,
      "未找到中文字体，请将 RATATUI_GUI_FONT 设置为中文 TTF/OTF/TTC 字体路径",
    )
  })?;
  let mut fonts = egui::FontDefinitions::default();
  fonts.font_data.insert(
    "cjk".into(),
    egui::FontData::from_owned(std::fs::read(path)?).into(),
  );
  for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
    fonts.families.entry(family).or_default().push("cjk".into());
  }
  ctx.set_fonts(fonts);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn contents(window: &Window) -> String {
    window
      .terminal
      .backend()
      .buffer
      .content
      .iter()
      .flat_map(|cell| cell.symbol().chars())
      .filter(|c| !c.is_whitespace())
      .collect()
  }

  #[test]
  fn window_events_drive_shared_buttons_dialog_and_resize() {
    let ctx = egui::Context::default();
    let mut window = Window {
      app: App::default(),
      terminal: Terminal::new(Grid::new(1, 1)).unwrap(),
      error: None,
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(880.0, 624.0));
    let mut tick = |events: Vec<egui::Event>, area: egui::Rect| {
      let output = ctx.run_ui(
        egui::RawInput {
          screen_rect: Some(area),
          events,
          ..Default::default()
        },
        |ui| {
          let ctx = ui.ctx();
          window.update_grid(ctx, area).unwrap();
          paint::grid(
            &ctx.layer_painter(egui::LayerId::background()),
            area,
            &window.terminal.backend().buffer,
            CELL,
            FONT_SIZE,
          );
        },
      );
      assert!(!output.shapes.is_empty());
      // This headless test inspects the grid without uploading GPU textures.
      output.drop_without_applying_deltas();
      contents(&window)
    };
    assert!(tick(vec![], rect).contains("当前计数：0"));
    let click = egui::Event::PointerButton {
      pos: egui::pos2(55.0, 270.0),
      button: egui::PointerButton::Primary,
      pressed: true,
      modifiers: egui::Modifiers::NONE,
    };
    assert!(tick(vec![click], rect).contains("当前计数：1"));
    assert!(tick(vec![egui::Event::Text("r".into())], rect).contains("确认重置"));
    let key = |key| egui::Event::Key {
      key,
      physical_key: None,
      pressed: true,
      repeat: false,
      modifiers: egui::Modifiers::NONE,
    };
    let text = tick(
      vec![key(egui::Key::ArrowRight), key(egui::Key::Enter)],
      rect,
    );
    assert!(text.contains("当前计数：0"));
    assert!(!text.contains("确认重置"));
    let tiny = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 120.0));
    tick(vec![], tiny);
    assert!(tick(vec![], rect).contains("当前计数：0"));
  }

  #[test]
  fn native_gpu_backend_is_enabled() {
    assert!(!wgpu::Instance::enabled_backend_features().is_empty());
  }
}
