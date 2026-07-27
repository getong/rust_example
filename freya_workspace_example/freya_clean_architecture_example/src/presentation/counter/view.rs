use freya::{
  components::Button,
  elements::rect::rect,
  prelude::{
    ChildrenExt, Component, ContainerSizeExt, ContainerWithContentExt, FontWeight, IntoElement,
    Size, StyleExt, TextStyleExt,
  },
  radio::{RadioReducer, use_radio},
};

use crate::presentation::counter::bloc::{CounterChannel, CounterEvent, CounterState};

#[derive(PartialEq)]
pub(crate) struct CounterView;

impl Component for CounterView {
  fn render(&self) -> impl IntoElement {
    let mut counter = use_radio::<CounterState, CounterChannel>(CounterChannel::Count);
    let count = counter.read().count();

    let counter_display = rect()
      .width(Size::fill())
      .height(Size::percent(50.))
      .center()
      .color((255, 255, 255))
      .background((15, 163, 242))
      .font_weight(FontWeight::BOLD)
      .font_size(75.)
      .shadow((0., 4., 20., 4., (0, 0, 0, 80)))
      .child(count.to_string());

    let actions = rect()
      .horizontal()
      .width(Size::fill())
      .height(Size::percent(50.))
      .center()
      .spacing(8.0)
      .child(
        Button::new()
          .on_press(move |_| {
            counter.apply(CounterEvent::Increment);
          })
          .child("Increase"),
      )
      .child(
        Button::new()
          .on_press(move |_| {
            counter.apply(CounterEvent::Decrement);
          })
          .child("Decrease"),
      );

    rect().child(counter_display).child(actions)
  }
}
