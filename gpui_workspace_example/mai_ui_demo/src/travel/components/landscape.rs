use gpui_kit::*;

use super::common::label;
use crate::travel::theme::GREEN;

pub(super) fn landscape() -> impl IntoElement {
  div()
    .relative()
    .h(px(175.))
    .w_full()
    .rounded_lg()
    .overflow_hidden()
    .bg(rgb(0xf0f5ee))
    .child(
      canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
          let p =
            |x: f32, y: f32| bounds.origin + point(bounds.size.width * x, bounds.size.height * y);
          for (color, points) in [
            (
              0xdce9df,
              vec![
                (0., 0.72),
                (0.13, 0.46),
                (0.24, 0.53),
                (0.43, 0.20),
                (0.64, 0.51),
                (0.77, 0.40),
                (1., 0.68),
                (1., 1.),
                (0., 1.),
              ],
            ),
            (
              0xb9d3c0,
              vec![
                (0., 0.83),
                (0.20, 0.65),
                (0.39, 0.72),
                (0.60, 0.49),
                (0.80, 0.64),
                (1., 0.55),
                (1., 1.),
                (0., 1.),
              ],
            ),
            (
              0xd2e6df,
              vec![
                (0., 0.86),
                (0.32, 0.79),
                (0.60, 0.81),
                (1., 0.73),
                (1., 1.),
                (0., 1.),
              ],
            ),
          ] {
            let mut path = PathBuilder::fill();
            for (index, (x, y)) in points.into_iter().enumerate() {
              if index == 0 {
                path.move_to(p(x, y));
              } else {
                path.line_to(p(x, y));
              }
            }
            path.close();
            if let Ok(path) = path.build() {
              window.paint_path(path, rgb(color));
            }
          }
          let mut route = PathBuilder::stroke(px(1.5));
          route.move_to(p(0.10, 0.91));
          route.curve_to(p(0.82, 0.85), p(0.46, 0.55));
          if let Ok(path) = route.build() {
            window.paint_path(path, rgb(0xf7faf5));
          }
        },
      )
      .size_full(),
    )
    .child(
      div()
        .absolute()
        .top_5()
        .right_8()
        .size_9()
        .rounded_full()
        .bg(rgb(0xf2c775)),
    )
    .child(label("苍 山", 10., GREEN).absolute().left_5().top_5())
    .child(
      label("洱 海  /  ERHAI", 10., GREEN)
        .absolute()
        .right_5()
        .bottom_3(),
    )
}
