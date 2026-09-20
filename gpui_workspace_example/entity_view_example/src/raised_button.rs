use gpui_kit::{base::Button, component::ActiveTheme, *};

use crate::palette::AppPalette;

/// 应用级立体按钮：语义、焦点与点击由 Base Button 管理。
#[derive(IntoElement)]
pub(crate) struct RaisedButton {
  button: Button,
  label: SharedString,
}

impl RaisedButton {
  pub(crate) fn new(id: impl Into<ElementId>) -> Self {
    Self {
      button: Button::new(id),
      label: SharedString::default(),
    }
  }

  pub(crate) fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = label.into();
    self
  }

  pub(crate) fn on_click(
    mut self,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.button = self.button.on_click(listener);
    self
  }
}

impl RenderOnce for RaisedButton {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let colors = AppPalette::default();
    // 阴影 API 使用物理像素；从当前 rem 换算，让阴影随应用字号缩放。
    let unit = window.rem_size();
    let shadows = |depth: f32, blur: f32| {
      vec![
        BoxShadow {
          inset: false,
          color: colors.depth,
          offset: point(px(0.), unit * depth),
          blur_radius: px(0.),
          spread_radius: px(0.),
        },
        BoxShadow {
          inset: false,
          color: colors.shadow,
          offset: point(px(0.), unit * (depth + 0.1875)),
          blur_radius: unit * blur,
          spread_radius: px(0.),
        },
      ]
    };
    // 保留固定占位，位移不会推动相邻内容；交互样式集中在按钮本身。
    div().relative().w_16().h_12().child(
      self
        .button
        .accessibility_label(self.label.clone())
        .absolute()
        .left_0()
        .top(rems(0.25))
        .w_full()
        .h(rems(2.125))
        .rounded(theme.radius)
        .border_t_1()
        .border_color(colors.edge)
        .bg(colors.button)
        .text_color(colors.foreground)
        .cursor_pointer()
        .shadow(shadows(0.25, 0.625))
        .hover(|style| {
          style
            .top(rems(0.125))
            .bg(colors.button_hover)
            .border_color(colors.edge_hover)
            .shadow(shadows(0.375, 1.))
        })
        .active(|style| {
          style
            .top(rems(0.4375))
            .bg(colors.button_active)
            .border_color(colors.edge_active)
            .shadow(shadows(0.0625, 0.1875))
        })
        .focus_visible(|style| style.border_2().border_color(theme.ring))
        .child(self.label),
    )
  }
}
