use gpui_kit::*;

pub(super) fn render(_window: &mut Window, _cx: &mut App) -> AnyElement {
  let preview = {
    // retain_all 用稳定 ID 保存缓存；子树中相同 Resource 的 Img 共用加载结果。
    let source = std::path::PathBuf::from(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/assets/gpui-elements-grid.png"
    ));
    image_cache(retain_all("element-image-cache"))
      .flex()
      .gap_4()
      .child(
        img(source.clone())
          .w(px(144.))
          .h(px(96.))
          .object_fit(ObjectFit::Contain),
      )
      .child(img(source).size(px(96.)).object_fit(ObjectFit::Cover))
  };
  preview.into_any_element()
}
