use gpui_kit::*;

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
  let preview = {
    let state = window.use_keyed_state("element-list-state", cx, |_, _| {
      ListState::new(1000, ListAlignment::Top, px(200.))
    });
    // ListState 跨帧保留滚动位置；行高可变。选择行为需要自己添加。
    div()
      .id("element-list-viewport")
      .test_support()
      .h(px(160.))
      .w_full()
      .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
      .child(
        list(state.read(cx).clone(), |index, _, _| {
          div()
            .h(px(28. + (index % 3) as f32 * 12.))
            .px_3()
            .border_b_1()
            .border_color(rgb(0xcbd5e1))
            .child(format!("可变高度行 {} / 1000", index + 1))
            .into_any_element()
        })
        .size_full(),
      )
  };
  preview.into_any_element()
}
