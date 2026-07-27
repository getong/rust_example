use freya::{
  components::Button,
  elements::{label::label, rect::rect},
  prelude::*,
  radio::{RadioReducer, use_radio},
};

use crate::presentation::counter::bloc::{
  CounterChannel, CounterEvent, CounterState, PersistenceStatus,
};

#[derive(PartialEq)]
pub(crate) struct StorageTab;

impl Component for StorageTab {
  fn render(&self) -> impl IntoElement {
    let mut counter = use_radio::<CounterState, CounterChannel>(CounterChannel::Persistence);
    let (count, storage_location, status_label, status_detail, status_color) = {
      let state = counter.read();
      let (label, detail, color) = match state.persistence_status() {
        PersistenceStatus::Loaded => ("Loaded", "Restored from disk".to_string(), (26, 127, 88)),
        PersistenceStatus::Saved => ("Saved", "Up to date on disk".to_string(), (26, 127, 88)),
        PersistenceStatus::Failed(error) => ("Unavailable", error.clone(), (184, 55, 55)),
      };

      (
        state.count(),
        state.storage_location().to_string(),
        label,
        detail,
        color,
      )
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
          .text("Storage"),
      )
      .child(
        rect()
          .width(Size::fill())
          .padding(16.)
          .spacing(6.)
          .corner_radius(6.)
          .background((255, 255, 255))
          .child(
            label()
              .color(status_color)
              .font_weight(FontWeight::BOLD)
              .text(status_label),
          )
          .child(paragraph().width(Size::fill()).span(status_detail)),
      )
      .child(storage_row("Current value", count.to_string()))
      .child(storage_row("File", storage_location))
      .child(rect().height(Size::flex(1.)))
      .child(
        rect()
          .width(Size::fill())
          .height(Size::px(60.))
          .center()
          .child(
            Button::new()
              .on_press(move |_| {
                counter.apply(CounterEvent::SaveNow);
              })
              .child("Save now"),
          ),
      )
  }
}

fn storage_row(label_text: impl Into<String>, value: impl Into<String>) -> impl IntoElement {
  rect()
    .width(Size::fill())
    .padding((12., 16.))
    .spacing(4.)
    .child(
      label()
        .font_weight(FontWeight::BOLD)
        .text(label_text.into()),
    )
    .child(paragraph().width(Size::fill()).span(value.into()))
}
