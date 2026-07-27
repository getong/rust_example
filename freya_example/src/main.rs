use freya::{
  components::Button,
  elements::rect::rect,
  prelude::{
    ChildrenExt, ContainerSizeExt, ContainerWithContentExt, FontWeight, IntoElement, LaunchConfig,
    Size, StyleExt, TextStyleExt, WindowConfig, launch, use_state,
  },
};

fn main() {
  // *Start* your app with a window and its root component
  launch(LaunchConfig::new().with_window(WindowConfig::new(app)))
}

fn app() -> impl IntoElement {
  let mut count = use_state(|| 4);

  let counter = rect()
    .width(Size::fill())
    .height(Size::percent(50.))
    .center()
    .color((255, 255, 255))
    .background((15, 163, 242))
    .font_weight(FontWeight::BOLD)
    .font_size(75.)
    .shadow((0., 4., 20., 4., (0, 0, 0, 80)))
    .child(count.read().to_string());

  let actions = rect()
    .horizontal()
    .width(Size::fill())
    .height(Size::percent(50.))
    .center()
    .spacing(8.0)
    .child(
      Button::new()
        .on_press(move |_| {
          *count.write() += 1;
        })
        .child("Increase"),
    )
    .child(
      Button::new()
        .on_press(move |_| {
          *count.write() -= 1;
        })
        .child("Decrease"),
    );

  rect().child(counter).child(actions)
}
// fn app() -> impl IntoElement {
//   // Define a reactive *state*
//   let mut count = use_state(|| 0);

//   // Declare the *UI*
//   rect()
//     .width(Size::fill())
//     .height(Size::fill())
//     .background((35, 35, 35))
//     .color(Color::WHITE)
//     .padding(Gaps::new_all(12.))
//     .on_mouse_up(move |_| *count.write() += 1)
//     .child(format!("Click to increase -> {}", count.read()))
// }
