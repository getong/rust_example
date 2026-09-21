use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let focus = window.use_keyed_state("stateful-focus", cx, |_, cx| cx.focus_handle());
    let focus_handle = focus.read(cx).clone();
    let target = focus_handle.clone();
    // id 返回 Stateful<Div>；焦点句柄保存在 Entity 中，不在每次 render 时重建。
    div()
      .id("element-stateful")
      .test_support()
      .track_focus(&focus_handle)
      .w(px(280.))
      .p_4()
      .border_2()
      .border_color(rgb(0x94a3b8))
      .rounded_lg()
      .cursor_pointer()
      .hover(|style| style.bg(rgb(0xdbeafe)))
      .focus(|style| style.border_color(rgb(0x2563eb)))
      .on_click(move |_, window, cx| target.focus(window, cx))
      .child("悬停看背景；点击获取焦点并改变边框")
  };
  preview.into_any_element()
}
