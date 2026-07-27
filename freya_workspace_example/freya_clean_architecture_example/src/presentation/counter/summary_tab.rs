use freya::{
  components::Button,
  elements::{label::label, rect::rect},
  prelude::*,
  radio::{RadioReducer, use_radio},
};

use crate::presentation::counter::bloc::{CounterChannel, CounterEvent, CounterState};

#[derive(PartialEq)]
pub(crate) struct SummaryTab;

impl Component for SummaryTab {
  fn render(&self) -> impl IntoElement {
    let mut counter = use_radio::<CounterState, CounterChannel>(CounterChannel::Count);
    let count = counter.read().count();
    let sign = match count.cmp(&0) {
      std::cmp::Ordering::Greater => "Positive",
      std::cmp::Ordering::Equal => "Zero",
      std::cmp::Ordering::Less => "Negative",
    };

    rect()
      .expanded()
      .content(Content::flex())
      .padding(24.)
      .spacing(16.)
      .child(
        label()
          .font_size(26.)
          .font_weight(FontWeight::BOLD)
          .text("Summary"),
      )
      .child(metric("Current value", count.to_string()))
      .child(metric("Sign", sign))
      .child(metric("Distance to zero", count.unsigned_abs().to_string()))
      .child(rect().height(Size::flex(1.)))
      .child(
        rect()
          .horizontal()
          .width(Size::fill())
          .height(Size::px(60.))
          .center()
          .spacing(8.)
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
          ),
      )
  }
}

fn metric(label_text: impl Into<String>, value: impl Into<String>) -> impl IntoElement {
  rect()
    .horizontal()
    .content(Content::flex())
    .width(Size::fill())
    .height(Size::px(56.))
    .padding((14., 16.))
    .corner_radius(6.)
    .background((255, 255, 255))
    .cross_align(Alignment::Center)
    .child(rect().width(Size::flex(1.)).child(label_text.into()))
    .child(label().font_weight(FontWeight::BOLD).text(value.into()))
}
