use gpui_kit::*;

// 当前版本只在 macOS 提供 surface()。调用方从视频解码器或相机取得
// CVPixelBuffer 后，使用 buffer.into() 传入本函数；此函数不负责采集视频。
#[cfg(target_os = "macos")]
pub(super) fn from_source(source: SurfaceSource) -> Surface {
  surface(source)
    .object_fit(ObjectFit::Contain)
    .w(px(320.))
    .h(px(180.))
}

pub(super) fn render(_: &mut Window, _: &mut App) -> AnyElement {
  #[cfg(target_os = "macos")]
  let _build_surface: fn(SurfaceSource) -> Surface = from_source;
  div()
    .p_4()
    .border_1()
    .border_color(rgb(0x94a3b8))
    .rounded_lg()
    .child(
      "Surface 接入示例已参与编译。当前未接入视频/相机的 \
       CVPixelBuffer，因此这里不显示原生视频画面。",
    )
    .into_any_element()
}
