//! 应用根视图 / Application root view.
//!
//! 左侧侧边栏切换图表功能页，右侧滚动区展示当前功能。

use gpui_kit::{
  Context, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
  Styled, Window,
  component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
  },
  div,
  prelude::FluentBuilder,
  px,
};

use crate::features::{self, ChartFeature};

pub struct ChartDemo {
  /// 当前选中的功能下标。
  active: usize,
}

impl ChartDemo {
  pub fn new() -> Self {
    Self { active: 0 }
  }
}

impl Default for ChartDemo {
  fn default() -> Self {
    Self::new()
  }
}

impl Render for ChartDemo {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let features: Vec<ChartFeature> = features::all();

    // 未启用任何图表 feature 时的占位。
    if features.is_empty() {
      return v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child("未启用任何图表 feature。请使用 `--features line,bar,...` 重新编译。")
        .into_any_element();
    }

    let active = self.active.min(features.len() - 1);

    // 先把需要的主题色拷出来，避免与 cx.listener 的可变借用冲突。
    let (border, muted_bg, muted_fg) = {
      let theme = cx.theme();
      (theme.border, theme.muted, theme.muted_foreground)
    };

    // ---- 侧边栏 ----
    let mut sidebar = v_flex()
      .w(px(240.0))
      .h_full()
      .flex_shrink_0()
      .border_r_1()
      .border_color(border)
      .bg(muted_bg)
      .p_3()
      .gap_1()
      .child(
        div()
          .font_weight(gpui_kit::FontWeight::SEMIBOLD)
          .p_2()
          .child("gpui-kit Chart 演示"),
      )
      .child(
        div()
          .text_xs()
          .text_color(muted_fg)
          .p_2()
          .pb_3()
          .child(format!("{} 个图表功能模块", features.len())),
      );

    for (index, feature) in features.iter().enumerate() {
      let selected = index == active;
      sidebar = sidebar.child(
        Button::new(feature.id)
          .label(feature.title)
          .w_full()
          .when(selected, |this| this.primary())
          .when(!selected, |this| this.ghost())
          .on_click(cx.listener(move |view, _, _, cx| {
            view.active = index;
            cx.notify();
          })),
      );
    }

    // ---- 内容区 ----
    let feature = &features[active];
    let feature_body = (feature.render)(cx);

    let content = v_flex()
      .id("chart-content")
      .flex_1()
      .h_full()
      .overflow_y_scroll()
      .p_6()
      .gap_4()
      .child(
        div()
          .font_weight(gpui_kit::FontWeight::SEMIBOLD)
          .text_lg()
          .child(feature.title),
      )
      .child(
        div()
          .text_sm()
          .text_color(muted_fg)
          .child(feature.description),
      )
      .child(feature_body);

    h_flex()
      .size_full()
      .child(sidebar)
      .child(content)
      .into_any_element()
  }
}
