use gpui_kit::{
  component::{ActiveTheme, IconName, IconNamed, StyledExt},
  *,
};

use super::ComponentPage;

pub struct GpuiElementsTab;

impl ComponentPage for GpuiElementsTab {
  fn title() -> &'static str {
    "GPUI 元素"
  }
  fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self)
  }
}

pub(super) fn lesson(
  title: &str,
  explanation: &str,
  code: impl Into<SharedString>,
  cx: &App,
) -> Div {
  div()
    .flex()
    .flex_col()
    .gap_3()
    .p_4()
    .w_full()
    .min_w_0()
    .border_1()
    .border_color(cx.theme().border)
    .rounded_lg()
    .child(div().text_lg().font_semibold().child(title.to_owned()))
    .child(div().text_sm().child(explanation.to_owned()))
    .child(
      div()
        .p_3()
        .rounded_md()
        .bg(cx.theme().muted)
        .text_sm()
        .child(code.into()),
    )
}

impl Render for GpuiElementsTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .flex_col()
      .gap_4()
      .w_full()
      .min_w_0()
      .child(
        "GPUI 是原生渲染框架，不是 HTML DOM。div() \
         负责布局、样式与事件；文本、图像、自定义绘制、虚拟列表由专门的元素负责。\
         下面预览均使用本项目 gpui-kit 重导出的 GPUI API。",
      )
      .child(
        lesson(
          "01 · 文本 / StyledText",
          "字符串可直接作为 child；StyledText 可为文本片段设置样式，InteractiveText \
           可增加链接等交互。参考 text.rs、input.rs。文本输入还需焦点、选区与 IME 支持，单纯的 \
           div 不等于输入框。",
          r#"div().child(StyledText::new("Hello GPUI · 你好"))"#,
          cx,
        )
        .child(StyledText::new("Hello GPUI · 你好")),
      )
      .child(
        lesson(
          "02 · svg() / img()",
          "svg() 用于可缩放图标；img() 用于照片、位图等图像内容。path 通过 AssetSource \
           读取；图片也可以来自本地路径或 HTTP。参考 \
           svg/svg.rs、image/image.rs、image_loading.rs。下方直接渲染已随应用打包的 \
           SVG，无需网络。",
          "svg().path(IconName::Info.path()).size(px(40.)).text_color(color)",
          cx,
        )
        .child(
          svg()
            .path(IconName::Info.path())
            .size(px(40.))
            .text_color(cx.theme().primary),
        ),
      )
      .child(
        lesson(
          "03 · canvas()",
          "拿到布局 bounds \
           后直接绘制，适合图表、路径、装饰。它不会自动提供按钮、选区或命中逻辑，这些通常由外层 \
           div 承担。参考 painting.rs、paths_bench.rs。",
          "canvas(|_, _, _| (), |bounds, _, window, _| window.paint_quad(fill(bounds, color)))",
          cx,
        )
        .child(
          canvas(
            |_, _, _| (),
            |bounds, _, window, _| {
              window.paint_quad(fill(bounds, rgb(0x2563eb)));
              let inner = Bounds {
                origin: bounds.origin + point(px(12.), px(12.)),
                size: size(bounds.size.width * 0.6, bounds.size.height - px(24.)),
              };
              window.paint_quad(fill(inner, rgb(0x38bdf8)));
            },
          )
          .w_full()
          .h(px(64.)),
        ),
      )
      .child(
        lesson(
          "04 · uniform_list() / list()",
          "uniform_list 适合等高行，list 配合 ListState \
           管理可变高度行。它们按可见范围生成内容，避免把大量 div 一次性全部布局。参考 \
           uniform_list.rs、list_example.rs。下方 1,000 行可独立滚动。",
          "uniform_list(id, 1000, cx.processor(|_, range: std::ops::Range<usize>, _, _| \
           range.map(render_row).collect())).h(px(160.))",
          cx,
        )
        .child(
          uniform_list(
            "primitive-list",
            1000,
            cx.processor(|_, range: std::ops::Range<usize>, _, _| {
              range
                .map(|i| {
                  div()
                    .h(px(32.))
                    .px_3()
                    .child(format!("虚拟行 {:04} · 只构建当前需要的行", i + 1))
                })
                .collect()
            }),
          )
          .h(px(160.))
          .w_full(),
        ),
      )
      .child(lesson(
        "05 · anchored() + deferred()",
        "anchored 决定浮层锚点和位置，可限制在窗口边界内；deferred \
         延迟绘制，使浮层在普通内容之后绘制。两者组合用于提示与弹出菜单；开关状态、\
         关闭事件仍需自己管理。参考 anchor.rs、popover.rs。",
        "deferred(anchored().anchor(Anchor::TopLeft).position(position).snap_to_window().\
         child(content))",
        cx,
      ))
      .child(lesson(
        "06 · Entity<View> / 自定义 Element",
        "实现 Render 的 Entity 持有跨帧状态，可直接作为 child；RenderOnce 适合组合现有元素；实现 \
         Element 则可接管 request_layout、prepaint、paint。参考 \
         input.rs、view_example/。Button、Input 等是上层组件，不是与 div 平级的 HTML 标签。",
        "cx.new(|cx| MyView::new(cx)) → Entity<MyView> → parent.child(view)",
        cx,
      ))
      .child(div().text_sm().child(
        "本地参考目录：/Users/gerald/test/rust/zed/crates/gpui/examples。示例用途以该目录为依据，\
         实际可编译 API 以本项目 gpui-kit 0.6.4 为准。",
      ))
  }
}
