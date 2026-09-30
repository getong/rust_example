use gpui_kit::{
  App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
  ScrollHandle, StatefulInteractiveElement, Styled, TestSupportExt as _, Window,
  component::{ActiveTheme, Selectable, button::Button},
  div, px, relative, rgb,
};

use super::ComponentPage;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ExtensionDemo {
  #[default]
  Layout,
  Scroll,
  Layers,
  Animation,
}

impl ExtensionDemo {
  const ALL: [(Self, &'static str); 4] = [
    (Self::Layout, "布局"),
    (Self::Scroll, "滚动列表"),
    (Self::Layers, "阴影与透明度"),
    (Self::Animation, "动画"),
  ];

  fn source(self) -> &'static str {
    match self {
      Self::Layout => include_str!("div_lab_tab/extension.rs"),
      Self::Scroll => include_str!("div_lab_tab/extension_scroll.rs"),
      Self::Layers => include_str!("div_lab_tab/extension_layers.rs"),
      Self::Animation => include_str!("div_lab_tab/extension_animation.rs"),
    }
  }
}

#[derive(Default)]
pub struct DivLabTab {
  shape: usize,
  scale_index: usize,
  scale_content: bool,
  enabled: bool,
  clicks: usize,
  animation_runs: usize,
  extension_demo: ExtensionDemo,
  source_scrolls: [ScrollHandle; 5],
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

fn demo_lesson(
  id: &'static str,
  title: &str,
  explanation: &str,
  code: String,
  scroll: &ScrollHandle,
  cx: &App,
) -> gpui_kit::Div {
  div()
    .flex()
    .flex_col()
    .gap_3()
    .p_4()
    .flex_shrink_0()
    .w_full()
    .min_w_0()
    .border_1()
    .border_color(cx.theme().border)
    .rounded_lg()
    .child(
      div()
        .text_lg()
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child(title.to_owned()),
    )
    .child(div().text_sm().child(explanation.to_owned()))
    .child(
      div()
        .id((gpui_kit::ElementId::from(id), "viewport"))
        // The inner native scroll handler runs first during bubbling. Stop here
        // so the same wheel gesture cannot also scroll the enclosing tab.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .test_support()
        .flex()
        .h(px(160.))
        .w_full()
        .flex_shrink_0()
        .child(
          crate::scroll_panel::ScrollPanel::new(id, scroll).child(
            div()
              .p_3()
              .bg(cx.theme().muted)
              .text_sm()
              .font_family("monospace")
              .child(code),
          ),
        ),
    )
}

impl DivLabTab {
  fn render_extension(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
    match self.extension_demo {
      ExtensionDemo::Layout => include!("div_lab_tab/extension.rs").into_any_element(),
      ExtensionDemo::Scroll => include!("div_lab_tab/extension_scroll.rs").into_any_element(),
      ExtensionDemo::Layers => include!("div_lab_tab/extension_layers.rs").into_any_element(),
      ExtensionDemo::Animation => include!("div_lab_tab/extension_animation.rs").into_any_element(),
    }
  }

  fn shape_metrics(&self) -> [f32; 5] {
    let scale = [1., 1.5, 2.][self.scale_index];
    let content_scale = if self.scale_content { scale } else { 1. };
    let (width, height, radius) = match self.shape {
      1 => (120., 48., 24.),
      2 => (72., 72., 36.),
      _ => (120., 64., 8.),
    };
    [
      width * scale,
      height * scale,
      radius * scale,
      8. * content_scale,
      14. * content_scale,
    ]
  }

  // Resolve the shared preview source into the code for the current frame.
  // Event handlers stay intact so the displayed code still demonstrates interaction.
  fn demo_codes(&self) -> [String; 5] {
    let [width, height, radius, padding, font_size] = self.shape_metrics();
    let shape = include_str!("div_lab_tab/shape.rs")
      .replace("px(width)", &format!("px({width:.1})"))
      .replace("px(height)", &format!("px({height:.1})"))
      .replace("px(radius)", &format!("px({radius:.1})"))
      .replace("px(padding)", &format!("px({padding:.1})"))
      .replace("px(font_size)", &format!("px({font_size:.1})"));
    let button =
      include_str!("div_lab_tab/button.rs").replace("self.clicks", &self.clicks.to_string());
    let switch = include_str!("div_lab_tab/switch.rs")
      .replace(
        "if self.enabled { 28. } else { 4. }",
        if self.enabled { "28.0" } else { "4.0" },
      )
      .replace("self.enabled", if self.enabled { "true" } else { "false" });
    let card = include_str!("div_lab_tab/card.rs")
      .replace(
        "(self.clicks % 11) as f32 / 10.",
        &format!("{:.1}", (self.clicks % 11) as f32 / 10.),
      )
      .replace(
        "self.clicks % 11 * 10",
        &(self.clicks % 11 * 10).to_string(),
      )
      .replace("self.enabled", if self.enabled { "true" } else { "false" });
    [
      format!(
        "// 当前尺寸 {width:.1} × {height:.1}，圆角 {radius:.1}，字号 {font_size:.1}，内边距 \
         {padding:.1}\nlet primary = cx.theme().primary;\nlet foreground = \
         cx.theme().primary_foreground;\nlet muted = cx.theme().muted;\n\n{shape}"
      ),
      format!(
        "// 当前点击次数：{}\nlet primary = cx.theme().primary;\nlet foreground = \
         cx.theme().primary_foreground;\n\n{button}",
        self.clicks
      ),
      format!(
        "// 当前开关：{}\nlet primary = cx.theme().primary;\n\n{switch}",
        if self.enabled { "开启" } else { "关闭" }
      ),
      format!(
        "// 当前进度：{}%，状态：{}\nlet primary = cx.theme().primary;\nlet muted = \
         cx.theme().muted;\n\n{card}",
        self.clicks % 11 * 10,
        if self.enabled {
          "运行中"
        } else {
          "待开始"
        }
      ),
      self
        .extension_demo
        .source()
        .replace("self.animation_runs", &self.animation_runs.to_string()),
    ]
  }
}

impl Render for DivLabTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let [width, height, radius, padding, font_size] = self.shape_metrics();
    let primary = cx.theme().primary;
    let foreground = cx.theme().primary_foreground;
    let muted = cx.theme().muted;
    let [
      shape_code,
      button_code,
      switch_code,
      card_code,
      extension_code,
    ] = self.demo_codes();
    div()
      .flex()
      .flex_col()
      .gap_4()
      .w_full()
      .min_w_0()
      .child(
        "div() 本身只是容器。尺寸决定布局，圆角与颜色决定外观，事件更新 Entity 状态，再通过 \
         cx.notify() \
         触发重绘。这里的“变形和放大”使用真实布局尺寸，不是把截图拉伸。每段代码下方是对应效果；\
         代码框右侧的滚动条可上下拖动，代码中的尺寸、位置、计数与进度均显示当前值，随操作更新；\
         回调保留原始交互逻辑。",
      )
      .child(
        demo_lesson(
          "div-source-shape",
          "01 · 形状与缩放实验",
          "切换矩形、胶囊、圆形并放大。仅放大盒子时字体不变；\
           同步放大时文字与内部间距也按比例变化。w/h 会重新参与布局；圆形要求宽高相等。",
          shape_code,
          &self.source_scrolls[0],
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
        .child(include!("div_lab_tab/shape.rs")),
      )
      .child(
        demo_lesson(
          "div-source-button",
          "02 · 外观 + 事件 → 按钮",
          "悬停与按下分别改变透明度，点击更新计数。id 提供稳定的交互标识，on_click \
           更新页面状态。本例演示鼠标交互；生产控件还应补齐键盘操作、焦点与无障碍语义，或使用现成 \
           Button。",
          button_code,
          &self.source_scrolls[1],
          cx,
        )
        .child(include!("div_lab_tab/button.rs")),
      )
      .child(
        demo_lesson(
          "div-source-switch",
          "03 · 嵌套 + 定位 + 状态 → 开关",
          "外层是胶囊轨道，内层是白色圆点；relative + absolute 定位，状态决定圆点的 left \
           和轨道颜色。点击切换。这里仍然只用了 div。",
          switch_code,
          &self.source_scrolls[2],
          cx,
        )
        .child(include!("div_lab_tab/switch.rs")),
      )
      .child(
        demo_lesson(
          "div-source-card",
          "04 · 容器组合 → 状态卡片与进度条",
          "卡片用 border、padding、flex_col；徽章用 rounded_full；进度条用轨道 div 嵌套填充 \
           div。点击上方按钮推进进度，第 10 次到 100%，第 11 次回到 0%；开关同步改变状态徽章。",
          card_code,
          &self.source_scrolls[3],
          cx,
        )
        .child(include!("div_lab_tab/card.rs")),
      )
      .child(
        demo_lesson(
          "div-source-extension",
          "05 · 继续扩展：布局、滚动、层次与动画",
          "点击按钮切换布局、滚动列表、阴影与透明度、动画。代码和下方效果同步切换，\
           代码滚动位置回到顶部；动画示例可重播。",
          extension_code,
          &self.source_scrolls[4],
          cx,
        )
        .child(
          div().flex().flex_wrap().gap_2().children(
            ExtensionDemo::ALL
              .into_iter()
              .enumerate()
              .map(|(index, (demo, label))| {
                Button::new(("div-extension-mode", index))
                  .label(label)
                  .selected(self.extension_demo == demo)
                  .on_click(cx.listener(move |this, _, _, cx| {
                    if this.extension_demo != demo {
                      this.extension_demo = demo;
                      this.source_scrolls[4].set_offset(gpui_kit::point(px(0.), px(0.)));
                      cx.notify();
                    }
                  }))
              }),
          ),
        )
        .child(self.render_extension(cx)),
      )
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
    let window = cx.open_window(size(px(1000.), px(3200.)), |window, cx| {
      Root::new(view.clone(), window, cx)
    });
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      let initial_code = view.read(cx).demo_codes();
      assert!(initial_code[0].contains(".w(px(120.0))"));
      window.click(("div-shape", 1usize), cx);
      assert!(view.read(cx).demo_codes()[0].contains(".h(px(48.0))"));
      window.click(("div-shape", 2usize), cx);
      assert!(view.read(cx).demo_codes()[0].contains(".w(px(72.0))"));
      window.click(("div-scale", 1usize), cx);
      assert!(view.read(cx).demo_codes()[0].contains(".w(px(108.0))"));
      window.click(("div-scale", 2usize), cx);
      let scaled = view.read(cx).demo_codes();
      assert!(scaled[0].contains(".w(px(144.0))"));
      assert!(scaled[0].contains(".text_size(px(28.0))"));
      window.click("div-scale-content", cx);
      window.click("div-counter", cx);
      window.click("div-switch", cx);
      let state = view.read(cx);
      assert_eq!((state.shape, state.scale_index, state.clicks), (2, 2, 1));
      assert!(!state.scale_content);
      assert!(state.enabled);
      let codes = state.demo_codes();
      assert!(codes[0].contains(".text_size(px(14.0))"));
      assert!(codes[0].contains(".px(px(8.0))"));
      assert!(codes[1].contains("点击这个 div · 已点击 {} 次\", 1"));
      assert!(codes[1].contains("this.clicks += 1"));
      assert!(codes[2].contains(".left(px(28.0))"));
      assert!(codes[2].contains("if true"));
      assert!(codes[2].contains("this.enabled = !this.enabled"));
      assert!(codes[3].contains(".w(relative(0.1))"));
      for _ in 0 .. 9 {
        window.click("div-counter", cx);
      }
      assert!(view.read(cx).demo_codes()[3].contains(".w(relative(1.0))"));
      window.click("div-counter", cx);
      assert!(view.read(cx).demo_codes()[3].contains(".w(relative(0.0))"));
      assert!(window.find("div-extension-layout").visible());
      for (index, preview_id) in [
        (1usize, "div-extension-list"),
        (2, "div-extension-layers"),
        (3, "div-extension-animation-preview"),
      ] {
        let previous_code = view.read(cx).demo_codes()[4].clone();
        window.click(("div-extension-mode", index), cx);
        assert!(window.find(preview_id).visible());
        assert!(window.try_find("div-extension-layout").is_none());
        assert_ne!(view.read(cx).demo_codes()[4], previous_code);
        assert!(view.read(cx).demo_codes()[4].contains(preview_id));
      }
      window.click("div-replay-animation", cx);
      assert_eq!(view.read(cx).animation_runs, 1);
      assert_ne!(view.read(cx).demo_codes()[4], initial_code[4]);
      window.scroll(
        (
          gpui_kit::ElementId::from("div-source-extension"),
          "viewport",
        ),
        gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-80.))),
        cx,
      );
      assert!(view.read(cx).source_scrolls[4].offset().y < px(0.));
      window.click(("div-extension-mode", 0usize), cx);
      assert_eq!(view.read(cx).source_scrolls[4].offset().y, px(0.));
      assert!(window.find("div-extension-layout").visible());
      assert!(window.try_find("div-replay-animation").is_none());
      assert_eq!(view.read(cx).demo_codes()[4], initial_code[4]);
      window.click("div-reset", cx);
      let state = view.read(cx);
      assert_eq!((state.shape, state.scale_index, state.clicks), (0, 0, 0));
      assert!(state.scale_content);
      assert!(!state.enabled);
      assert_eq!(state.demo_codes(), initial_code);
    })
    .unwrap();
  }

  #[gpui_kit::test]
  fn code_wheel_scroll_does_not_move_parent(cx: &mut TestAppContext) {
    use gpui_kit::{
      Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, Render,
      ScrollDelta, ScrollHandle, StatefulInteractiveElement, Styled, TestSupportExt, Window, div,
      point,
    };

    struct NestedPage {
      lab: Entity<DivLabTab>,
      outer: ScrollHandle,
    }
    impl Render for NestedPage {
      fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
          .id("outer-scroll")
          .size_full()
          .overflow_y_scroll()
          .track_scroll(&self.outer)
          .child(
            div()
              .id("outside-code")
              .test_support()
              .h(px(50.))
              .child("Tab 内容"),
          )
          .child(self.lab.clone())
      }
    }

    cx.update(gpui_kit::init);
    let lab = cx.new(|_| DivLabTab {
      scale_content: true,
      ..Default::default()
    });
    let outer = ScrollHandle::new();
    let page = cx.new(|_| NestedPage {
      lab: lab.clone(),
      outer: outer.clone(),
    });
    let window = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
      Root::new(page, window, cx)
    });
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      let code = lab.read(cx).source_scrolls[0].clone();
      let code_id = (ElementId::from("div-source-shape"), "viewport");
      window.scroll(
        code_id.clone(),
        ScrollDelta::Pixels(point(px(0.), px(-80.))),
        cx,
      );
      assert!(code.offset().y < px(0.), "code must scroll first");
      assert_eq!(outer.offset().y, px(0.), "parent must stay still over code");
      // Even at the code boundary, keep wheel events inside its viewport.
      for _ in 0 .. 2 {
        window.scroll(
          code_id.clone(),
          ScrollDelta::Pixels(point(px(0.), px(-10000.))),
          cx,
        );
      }
      assert_eq!(outer.offset().y, px(0.));
      let bottom = code.offset().y;
      window.scroll(code_id, ScrollDelta::Pixels(point(px(0.), px(40.))), cx);
      assert!(code.offset().y > bottom);
      window.scroll(
        "outside-code",
        ScrollDelta::Pixels(point(px(0.), px(-80.))),
        cx,
      );
      assert!(
        outer.offset().y < px(0.),
        "outside code the tab must scroll"
      );
    })
    .unwrap();
  }
}
