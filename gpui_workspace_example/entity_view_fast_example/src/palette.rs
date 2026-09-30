use gpui_kit::{Hsla, rgb, rgba};

/// 应用配色：保留组件重构前的深色画布及立体按钮颜色。
/// 不修改库的全局主题，滚动区和通知继续使用原有组件主题。
pub(crate) struct AppPalette {
  pub background: Hsla,
  pub foreground: Hsla,
  pub button: Hsla,
  pub button_hover: Hsla,
  pub button_active: Hsla,
  pub edge: Hsla,
  pub edge_hover: Hsla,
  pub edge_active: Hsla,
  pub depth: Hsla,
  pub shadow: Hsla,
}

impl Default for AppPalette {
  fn default() -> Self {
    Self {
      background: rgb(0x1e1e1e).into(),
      foreground: rgb(0xffffff).into(),
      button: rgb(0x343840).into(),
      button_hover: rgb(0x474e59).into(),
      button_active: rgb(0x292d34).into(),
      edge: rgb(0x777d87).into(),
      edge_hover: rgb(0xa6afbd).into(),
      edge_active: rgb(0x505661).into(),
      depth: rgb(0x101114).into(),
      shadow: rgba(0x00000080).into(),
    }
  }
}
