use gpui_kit::{
  component::{
    ActiveTheme,
    button::Button,
    plot::{
      scale::{Scale, ScaleLinear},
      shape::Line,
    },
    v_flex,
  },
  *,
};
pub struct PlotTab {
  alternate: bool,
}
impl Render for PlotTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let values = if self.alternate {
      vec![30., 80., 25., 60., 90., 50.]
    } else {
      vec![10., 40., 30., 80., 60., 95.]
    };
    let color = cx.theme().chart_1;
    v_flex()
      .gap_4()
      .child("Plot primitives: ScaleLinear maps data to pixels; Line paints the curve.")
      .child(
        Button::new("plot-change")
          .label("Change data")
          .on_click(cx.listener(|this, _, _, cx| {
            this.alternate = !this.alternate;
            cx.notify();
          })),
      )
      .child(
        canvas(
          |_, _, _| (),
          move |bounds, _, window, _| {
            let x = ScaleLinear::new(vec![0., 5.], vec![0., f32::from(bounds.size.width)]);
            let y = ScaleLinear::new(vec![0., 100.], vec![f32::from(bounds.size.height), 0.]);
            Line::new()
              .data(values.iter().enumerate().map(|(i, v)| (i as f64, *v)))
              .x(move |p| x.tick(&p.0))
              .y(move |p| y.tick(&p.1))
              .stroke(color)
              .stroke_width(px(2.))
              .dot()
              .paint(&bounds, window);
          },
        )
        .w_full()
        .h_64(),
      )
  }
}
impl super::ComponentPage for PlotTab {
  fn title() -> &'static str {
    "Plot"
  }
  fn new_view(_window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self { alternate: false })
  }
}
