use gpui_kit::{
  App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, Styled, TestSupportExt as _, Window,
  component::{ActiveTheme, Selectable, button::Button},
  div, px, relative, rgb,
};

use super::{ComponentPage, gpui_elements_tab::lesson};

#[derive(Default)]
pub struct DivLabTab {
  shape: usize,
  scale_index: usize,
  scale_content: bool,
  enabled: bool,
  clicks: usize,
}

impl ComponentPage for DivLabTab {
  fn title() -> &'static str {
    "div() 实验室"
  }
  fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self {
      scale_content: true,
      ..Self::default()
    })
  }
}

impl Render for DivLabTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let scale = [1., 1.5, 2.][self.scale_index];
    let content_scale = if self.scale_content { scale } else { 1. };
    let (width, height, radius) = match self.shape {
      1 => (120., 48., 24.),
      2 => (72., 72., 36.),
      _ => (120., 64., 8.),
    };
    let primary = cx.theme().primary;
    let foreground = cx.theme().primary_foreground;
    let muted = cx.theme().muted;
    div()
      .flex()
      .flex_col()
      .gap_4()
      .w_full()
      .min_w_0()
      .child(
        "div() 本身只是容器。尺寸决定布局，圆角与颜色决定外观，事件更新 Entity 状态，再通过 \
         cx.notify() 触发重绘。这里的“变形和放大”使用真实布局尺寸，不是把截图拉伸。",
      )
      .child(
        lesson(
          "01 · 形状与缩放实验",
          "切换矩形、胶囊、圆形并放大。仅放大盒子时字体不变；\
           同步放大时文字与内部间距也按比例变化。w/h 会重新参与布局；圆形要求宽高相等。",
          format!(
            "div().w(px({:.0})).h(px({:.0})).rounded(px({:.0})).text_size(px({:.0})).px(px({:.0}))",
            width * scale,
            height * scale,
            radius * scale,
            14. * content_scale,
            8. * content_scale
          ),
          cx,
        )
        .child(
          div().flex().flex_wrap().gap_2().children(
            ["矩形", "胶囊", "圆形"]
              .into_iter()
              .enumerate()
              .map(|(i, name)| {
                Button::new(("div-shape", i))
                  .label(name)
                  .selected(self.shape == i)
                  .on_click(cx.listener(move |this, _, _, cx| {
                    this.shape = i;
                    cx.notify();
                  }))
              }),
          ),
        )
        .child(
          div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(
              ["1×", "1.5×", "2×"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                  Button::new(("div-scale", i))
                    .label(name)
                    .selected(self.scale_index == i)
                    .on_click(cx.listener(move |this, _, _, cx| {
                      this.scale_index = i;
                      cx.notify();
                    }))
                }),
            )
            .child(
              Button::new("div-scale-content")
                .label(if self.scale_content {
                  "同步放大文字 / 间距"
                } else {
                  "仅放大盒子"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                  this.scale_content = !this.scale_content;
                  cx.notify();
                })),
            )
            .child(Button::new("div-reset").label("重置").on_click(cx.listener(
              |this, _, _, cx| {
                *this = Self {
                  scale_content: true,
                  ..Self::default()
                };
                cx.notify();
              },
            ))),
        )
        .child(
          div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(184.))
            .w_full()
            .bg(muted)
            .rounded_lg()
            .child(
              div()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(width * scale))
                .h(px(height * scale))
                .rounded(px(radius * scale))
                .px(px(8. * content_scale))
                .text_size(px(14. * content_scale))
                .bg(primary)
                .text_color(foreground)
                .child("div()"),
            ),
        ),
      )
      .child(
        lesson(
          "02 · 外观 + 事件 → 按钮",
          "悬停改变颜色，按下改变透明度，点击更新计数。id 提供稳定的交互标识，on_click \
           更新页面状态。本例演示鼠标交互；生产控件还应补齐键盘操作、焦点与无障碍语义，或使用现成 \
           Button。",
          "div().id(id).rounded_lg().bg(color).hover(...).active(...).on_click(cx.listener(...))",
          cx,
        )
        .child(
          div()
            .id("div-counter")
            .test_support()
            .cursor_pointer()
            .px_4()
            .py_3()
            .rounded_lg()
            .bg(primary)
            .text_color(foreground)
            .hover(|style| style.opacity(0.85))
            .active(|style| style.opacity(0.65))
            .on_click(cx.listener(|this, _, _, cx| {
              this.clicks += 1;
              cx.notify();
            }))
            .child(format!("点击这个 div · 已点击 {} 次", self.clicks)),
        ),
      )
      .child(
        lesson(
          "03 · 嵌套 + 定位 + 状态 → 开关",
          "外层是胶囊轨道，内层是白色圆点；relative + absolute 定位，状态决定圆点的 left \
           和轨道颜色。点击切换。这里仍然只用了 div。",
          "div().relative().w(px(56.)).h(px(32.)).rounded_full().child(div().absolute().\
           left(px(if enabled { 28. } else { 4. })).size(px(24.)))",
          cx,
        )
        .child(
          div()
            .flex()
            .items_center()
            .gap_3()
            .child(
              div()
                .id("div-switch")
                .test_support()
                .cursor_pointer()
                .relative()
                .w(px(56.))
                .h(px(32.))
                .rounded_full()
                .bg(if self.enabled {
                  primary
                } else {
                  cx.theme().border
                })
                .on_click(cx.listener(|this, _, _, cx| {
                  this.enabled = !this.enabled;
                  cx.notify();
                }))
                .child(
                  div()
                    .absolute()
                    .top(px(4.))
                    .left(px(if self.enabled { 28. } else { 4. }))
                    .size(px(24.))
                    .rounded_full()
                    .bg(rgb(0xffffff)),
                ),
            )
            .child(if self.enabled {
              "已开启"
            } else {
              "已关闭"
            }),
        ),
      )
      .child(
        lesson(
          "04 · 容器组合 → 状态卡片与进度条",
          "卡片用 border、padding、flex_col；徽章用 rounded_full；进度条用轨道 div 嵌套填充 \
           div。点击上方按钮推进进度，每 10 次循环；开关同步改变状态徽章。",
          "div().flex().flex_col().gap_3().p_4().border_1() … div().w(relative(progress)).h_full()",
          cx,
        )
        .child(
          div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .child(
              div()
                .flex()
                .flex_wrap()
                .gap_3()
                .items_center()
                .justify_between()
                .child("由 div 组合的任务卡片")
                .child(
                  div()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(muted)
                    .child(if self.enabled {
                      "运行中"
                    } else {
                      "待开始"
                    }),
                ),
            )
            .child(format!(
              "进度 {}% · 来自按钮的点击状态",
              self.clicks % 11 * 10
            ))
            .child(
              div()
                .h(px(12.))
                .w_full()
                .rounded_full()
                .overflow_hidden()
                .bg(muted)
                .child(
                  div()
                    .h_full()
                    .w(relative((self.clicks % 11) as f32 / 10.))
                    .bg(primary),
                ),
            ),
        ),
      )
      .child(lesson(
        "05 · 继续扩展",
        "flex / flex_col / gap 组织布局；overflow_y_scroll \
         配合有限高度形成滚动区；shadow、border、opacity 表达层级；with_animation \
         可逐帧改变尺寸与样式。放大父 div 不会自动放大所有后代，固定 px 值需要同步调整。",
        "参考：hello_world.rs · shadow.rs · opacity.rs · scrollable.rs · animation.rs · anchor.rs",
        cx,
      ))
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};

  use super::DivLabTab;

  #[gpui_kit::test]
  fn div_controls_update_state_and_reset(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx.new(|_| DivLabTab {
      scale_content: true,
      ..Default::default()
    });
    let window = cx.open_window(size(px(1000.), px(1800.)), |window, cx| {
      Root::new(view.clone(), window, cx)
    });
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      window.click(("div-shape", 2usize), cx);
      window.click(("div-scale", 2usize), cx);
      window.click("div-scale-content", cx);
      window.click("div-counter", cx);
      window.click("div-switch", cx);
      let state = view.read(cx);
      assert_eq!((state.shape, state.scale_index, state.clicks), (2, 2, 1));
      assert!(!state.scale_content);
      assert!(state.enabled);
      window.click("div-reset", cx);
      let state = view.read(cx);
      assert_eq!((state.shape, state.scale_index, state.clicks), (0, 0, 0));
      assert!(state.scale_content);
      assert!(!state.enabled);
    })
    .unwrap();
  }
}
