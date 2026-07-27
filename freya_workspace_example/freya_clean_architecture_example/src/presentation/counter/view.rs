use freya::{
  components::Button,
  elements::{label::label, rect::rect},
  prelude::{
    ChildrenExt, Component, ContainerExt, ContainerSizeExt, ContainerWithContentExt, Content,
    FontWeight, IntoElement, Size, StyleExt, TextStyleExt,
  },
  radio::{RadioReducer, use_radio},
};

use crate::presentation::counter::bloc::{CounterChannel, CounterEvent, CounterState};

#[derive(PartialEq)]
pub(crate) struct CounterTab;

impl Component for CounterTab {
  fn render(&self) -> impl IntoElement {
    let mut counter = use_radio::<CounterState, CounterChannel>(CounterChannel::Count);
    let count = counter.read().count();

    let counter_display = rect()
      .width(Size::fill())
      .height(Size::flex(1.))
      .center()
      .color((255, 255, 255))
      .background((15, 163, 242))
      .corner_radius(6.)
      .font_weight(FontWeight::BOLD)
      .font_size(75.)
      .child(count.to_string());

    let actions = rect()
      .horizontal()
      .width(Size::fill())
      .height(Size::px(60.))
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

    rect()
      .expanded()
      .content(Content::flex())
      .padding(24.)
      .spacing(16.)
      .child(
        label()
          .font_size(26.)
          .font_weight(FontWeight::BOLD)
          .text("Counter"),
      )
      .child(counter_display)
      .child(actions)
  }
}
