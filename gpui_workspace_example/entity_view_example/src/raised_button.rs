use gpui_kit::{
  component::button::{Button, ButtonCustomVariant, ButtonVariants},
  *,
};

fn depth_shadow(depth: f32, blur: f32) -> Vec<BoxShadow> {
  vec![
    BoxShadow {
      inset: false,
      color: rgb(0x101114).into(),
      offset: point(px(0.), px(depth)),
      blur_radius: px(0.),
      spread_radius: px(0.),
    },
    BoxShadow {
      inset: false,
      color: rgba(0x00000080).into(),
      offset: point(px(0.), px(depth + 3.)),
      blur_radius: px(blur),
      spread_radius: px(0.),
    },
  ]
}

pub(crate) fn raised_button(
  id: &'static str,
  label: String,
  cx: &App,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
  // 固定占位；仅移动外壳，不改变计数面板的布局。
  div().relative().w(px(64.)).h(px(48.)).child(
    div()
      .id((ElementId::from(id), "depth"))
      .absolute()
      .top(px(4.))
      .left_0()
      .w_full()
      .rounded(px(9.))
      .border_t_1()
      .border_color(rgb(0x777d87))
      .bg(rgb(0x343840))
      .shadow(depth_shadow(4., 10.))
      .hover(|style| {
        style
          .top(px(2.))
          .border_color(rgb(0xa6afbd))
          .shadow(depth_shadow(6., 16.))
      })
      .active(|style| {
        style
          .top(px(7.))
          .border_color(rgb(0x505661))
          .shadow(depth_shadow(1., 3.))
      })
      .child(
        Button::new(id)
          .custom(
            ButtonCustomVariant::new(cx)
              .color(rgb(0x343840).into())
              .hover(rgb(0x474e59).into())
              .active(rgb(0x292d34).into())
              .foreground(rgb(0xffffff).into()),
          )
          .rounded(px(8.))
          .w_full()
          .h(px(34.))
          .cursor_pointer()
          .label(label)
          .on_click(on_click),
      ),
  )
}
