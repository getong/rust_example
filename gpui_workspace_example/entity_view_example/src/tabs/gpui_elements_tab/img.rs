use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    let source = std::path::PathBuf::from(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/assets/gpui-elements-grid.png"
    ));
    div()
      .flex()
      .flex_wrap()
      .gap_4()
      .child(
        img(source.clone())
          .w(px(160.))
          .h(px(100.))
          .object_fit(ObjectFit::Contain),
      )
      .child(
        img(source)
          .w(px(100.))
          .h(px(100.))
          .object_fit(ObjectFit::Cover),
      )
  };
  preview.into_any_element()
}
