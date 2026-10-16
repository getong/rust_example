use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
  Frame,
  layout::{Constraint, Layout, Position, Rect},
  style::{Color, Style, Stylize},
  widgets::{Block, Clear, Paragraph, Wrap},
};

const LABELS: [&str; 4] = ["增加计数", "重置计数", "关于", "退出"];
const TIPS: [&str; 4] = [
  "增加计数：每次点击加 1。快捷键：+",
  "重置计数：打开确认框，确认后清零。快捷键：r",
  "关于：了解这个终端交互示例。快捷键：?",
  "退出：关闭示例并恢复终端。快捷键：q",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
  Reset,
  About,
}

pub struct App {
  pub quit: bool,
  count: u64,
  focus: usize,
  hover: Option<usize>,
  dialog: Option<Dialog>,
  dialog_focus: usize,
  buttons: [Rect; 4],
  dialog_buttons: [Rect; 2],
  status: &'static str,
  compact: bool,
}

impl Default for App {
  fn default() -> Self {
    Self {
      quit: false,
      count: 0,
      focus: 0,
      hover: None,
      dialog: None,
      dialog_focus: 0,
      buttons: [Rect::default(); 4],
      dialog_buttons: [Rect::default(); 2],
      status: "准备就绪。点击按钮或使用键盘操作。",
      compact: false,
    }
  }
}

impl App {
  pub fn handle(&mut self, event: Event) {
    match event {
      Event::Key(key) if key.kind == KeyEventKind::Press => {
        self.hover = None;
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
          self.quit = true;
          return;
        }
        if self.compact {
          if key.code == KeyCode::Char('q') {
            self.quit = true;
          }
          return;
        }
        if let Some(dialog) = self.dialog {
          match key.code {
            KeyCode::Esc => self.close_dialog(),
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right
              if dialog == Dialog::Reset =>
            {
              self.dialog_focus ^= 1
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.accept_dialog(),
            _ => {}
          }
          return;
        }
        match key.code {
          KeyCode::Tab | KeyCode::Right | KeyCode::Down => self.focus = (self.focus + 1) % 4,
          KeyCode::BackTab | KeyCode::Left | KeyCode::Up => self.focus = (self.focus + 3) % 4,
          KeyCode::Enter | KeyCode::Char(' ') => self.activate(self.focus),
          KeyCode::Char('+') | KeyCode::Char('=') => self.activate(0),
          KeyCode::Char('r') => self.activate(1),
          KeyCode::Char('?') => self.activate(2),
          KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
          _ => {}
        }
      }
      Event::Mouse(mouse) if !self.compact => {
        let point = Position::new(mouse.column, mouse.row);
        let areas: &[Rect] = if self.dialog.is_some() {
          &self.dialog_buttons
        } else {
          &self.buttons
        };
        let hit = areas.iter().position(|area| area.contains(point));
        if mouse.kind == MouseEventKind::Moved {
          if self.dialog.is_some() {
            if let Some(index) = hit {
              self.dialog_focus = index;
            }
          } else {
            self.hover = hit;
          }
        } else if mouse.kind == MouseEventKind::Down(MouseButton::Left)
          && let Some(index) = hit
        {
          if self.dialog.is_some() {
            self.dialog_focus = index;
            self.accept_dialog();
          } else {
            self.focus = index;
            self.activate(index);
          }
        }
      }
      Event::Resize(_, _) => self.hover = None,
      _ => {}
    }
  }

  fn activate(&mut self, index: usize) {
    match index {
      0 => {
        self.count = self.count.saturating_add(1);
        self.status = "已增加计数。试试重置按钮的确认弹窗。";
      }
      1 => self.dialog = Some(Dialog::Reset),
      2 => self.dialog = Some(Dialog::About),
      3 => self.quit = true,
      _ => {}
    }
    // Default to Cancel when opening a destructive confirmation.
    self.dialog_focus = 0;
    self.hover = None;
  }

  fn close_dialog(&mut self) {
    self.dialog = None;
    self.hover = None;
  }

  fn accept_dialog(&mut self) {
    if self.dialog == Some(Dialog::Reset) {
      if self.dialog_focus == 1 {
        self.count = 0;
        self.status = "计数已重置为 0。";
      } else {
        self.status = "已取消重置，计数保持不变。";
      }
    }
    self.close_dialog();
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    self.buttons = [Rect::default(); 4];
    self.dialog_buttons = [Rect::default(); 2];
    self.compact = area.width < 52 || area.height < 22;
    frame.render_widget(Block::default().bg(Color::Rgb(17, 24, 39)), area);
    if self.compact {
      frame.render_widget(
        Paragraph::new("请将终端调整到至少 52 列 × 22 行。\n按 q 或 Ctrl+C 退出。")
          .white()
          .wrap(Wrap { trim: false }),
        area,
      );
      return;
    }
    let [header, counter, buttons, hint, status, footer] = Layout::vertical([
      Constraint::Length(3),
      Constraint::Length(5),
      Constraint::Length(3),
      Constraint::Length(4),
      Constraint::Min(3),
      Constraint::Length(2),
    ])
    .margin(1)
    .areas(area);
    frame.render_widget(
      Paragraph::new("RATATUI  /  终端交互实验室\n按钮 · 悬停提示 · 模态弹窗 · 键盘导航")
        .cyan()
        .bold(),
      header,
    );
    frame.render_widget(
      Paragraph::new(format!("\n当前计数：{}", self.count))
        .centered()
        .white()
        .block(
          Block::bordered()
            .title(" 演示面板 ")
            .border_style(Color::DarkGray),
        ),
      counter,
    );
    self.buttons = Layout::horizontal([Constraint::Ratio(1, 4); 4])
      .spacing(1)
      .areas(buttons);
    for (index, label) in LABELS.iter().enumerate() {
      button(
        frame,
        self.buttons[index],
        label,
        self.dialog.is_none() && (self.hover.unwrap_or(self.focus) == index),
      );
    }
    frame.render_widget(
      Paragraph::new(TIPS[self.hover.unwrap_or(self.focus)])
        .wrap(Wrap { trim: false })
        .yellow()
        .block(
          Block::bordered()
            .title(" 按钮提示 ")
            .border_style(Color::DarkGray),
        ),
      hint,
    );
    frame.render_widget(
      Paragraph::new(self.status)
        .wrap(Wrap { trim: false })
        .green()
        .block(
          Block::bordered()
            .title(" 操作反馈 ")
            .border_style(Color::DarkGray),
        ),
      status,
    );
    frame.render_widget(
      Paragraph::new(
        "Tab / 方向键 切换  Enter / 空格 执行\n鼠标悬停查看提示 · Esc 关闭弹窗 · q 退出",
      )
      .style(Style::default().fg(Color::Gray)),
      footer,
    );
    if let Some(dialog) = self.dialog {
      self.draw_dialog(frame, dialog);
    } else if let Some(index) = self.hover {
      let tip_area = Rect::new(hint.x, hint.y, hint.width.min(64), 4);
      frame.render_widget(Clear, tip_area);
      frame.render_widget(
        Paragraph::new(TIPS[index])
          .wrap(Wrap { trim: false })
          .black()
          .on_yellow()
          .block(Block::bordered().title(" 悬停提示 Tooltip ")),
        tip_area,
      );
    }
  }

  fn draw_dialog(&mut self, frame: &mut Frame, dialog: Dialog) {
    let area = frame
      .area()
      .centered(Constraint::Length(58), Constraint::Length(12));
    let (title, text) = match dialog {
      Dialog::Reset => (
        " 确认重置 ",
        "是否将计数重置为 0？\n此操作会清除当前计数。\n\nTab / 左右键选择，Enter 执行，Esc 取消。",
      ),
      Dialog::About => (
        " 关于示例 ",
        "Ratatui 绘制终端界面（TUI）。\n本示例使用 Crossterm \
         接收键盘和鼠标事件。\n\n支持按钮、提示文字和阻止背景操作的弹窗。",
      ),
    };
    frame.render_widget(Clear, area);
    let block = Block::bordered()
      .title(title)
      .border_style(Color::Cyan)
      .on_black();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [body, actions] = Layout::vertical([Constraint::Min(1), Constraint::Length(3)])
      .margin(1)
      .areas(inner);
    frame.render_widget(
      Paragraph::new(text).white().wrap(Wrap { trim: false }),
      body,
    );
    if dialog == Dialog::Reset {
      self.dialog_buttons = Layout::horizontal([Constraint::Percentage(50); 2])
        .spacing(2)
        .areas(actions);
      button(
        frame,
        self.dialog_buttons[0],
        "取消",
        self.dialog_focus == 0,
      );
      button(
        frame,
        self.dialog_buttons[1],
        "确认重置",
        self.dialog_focus == 1,
      );
    } else {
      self.dialog_buttons[0] = actions;
      button(frame, actions, "知道了 / Esc", true);
    }
  }
}

fn button(frame: &mut Frame, area: Rect, label: &str, selected: bool) {
  let style = if selected {
    Style::default().black().on_cyan().bold()
  } else {
    Style::default().white().bg(Color::Rgb(31, 41, 55))
  };
  frame.render_widget(
    Paragraph::new(label)
      .centered()
      .style(style)
      .block(Block::bordered()),
    area,
  );
}

#[cfg(test)]
mod tests {
  use crossterm::event::{KeyEvent, MouseEvent};
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  fn key(app: &mut App, code: KeyCode) {
    app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
  }

  fn mouse(app: &mut App, area: Rect, kind: MouseEventKind) {
    app.handle(Event::Mouse(MouseEvent {
      kind,
      column: area.x + 1,
      row: area.y + 1,
      modifiers: KeyModifiers::NONE,
    }));
  }

  fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    terminal
      .backend()
      .buffer()
      .content
      .iter()
      // Wide Chinese glyphs leave a blank continuation cell in the buffer.
      .flat_map(|cell| cell.symbol().chars())
      .filter(|ch| !ch.is_whitespace())
      .collect()
  }

  #[test]
  fn reset_requires_confirmation_and_modal_blocks_background() {
    let mut app = App::default();
    key(&mut app, KeyCode::Char('+'));
    key(&mut app, KeyCode::Char('r'));
    key(&mut app, KeyCode::Char('+'));
    key(&mut app, KeyCode::Char('q'));
    assert_eq!(app.count, 1);
    assert!(!app.quit);
    key(&mut app, KeyCode::Enter); // Default Cancel.
    assert_eq!(app.count, 1);
    assert_eq!(app.dialog, None);
    key(&mut app, KeyCode::Char('r'));
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.count, 0);
    assert_eq!(app.dialog, None);
  }

  #[test]
  fn keyboard_focus_wraps_and_escape_closes_about() {
    let mut app = App::default();
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.focus, 3);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, 0);
    key(&mut app, KeyCode::Char('?'));
    assert!(render(&mut app, 80, 24).contains("关于示例"));
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.dialog, None);
    assert!(!app.quit);
  }

  #[test]
  fn mouse_hover_click_and_modal_confirmation() {
    let mut app = App::default();
    render(&mut app, 80, 24);
    let add = app.buttons[0];
    mouse(&mut app, add, MouseEventKind::Moved);
    assert!(render(&mut app, 80, 24).contains("悬停提示Tooltip"));
    mouse(&mut app, add, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(app.count, 1);
    let reset = app.buttons[1];
    mouse(&mut app, reset, MouseEventKind::Down(MouseButton::Left));
    assert!(render(&mut app, 80, 24).contains("确认重置"));
    // A background click must not activate the covered main button.
    mouse(&mut app, add, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(app.count, 1);
    assert_eq!(app.dialog, Some(Dialog::Reset));
    let confirm = app.dialog_buttons[1];
    mouse(&mut app, confirm, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(app.count, 0);
    assert_eq!(app.dialog, None);
  }

  #[test]
  fn resize_clears_hit_targets_and_preserves_modal() {
    let mut app = App::default();
    key(&mut app, KeyCode::Char('r'));
    assert!(render(&mut app, 52, 22).contains("确认重置"));
    render(&mut app, 10, 5);
    assert!(app.compact);
    assert!(app.buttons.iter().all(|area| area.is_empty()));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.dialog, Some(Dialog::Reset));
    assert!(render(&mut app, 100, 30).contains("确认重置"));
    render(&mut app, 0, 0);
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
  }
}
