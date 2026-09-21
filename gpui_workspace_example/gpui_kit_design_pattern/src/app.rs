//! 装配层：创建独立示例；具体模式的状态及订阅留在各自模块。
use gpui_kit::{component::ActiveTheme, *};

use crate::{
  composition::Counter,
  events::EventDemo,
  global_state::GlobalDemo,
  observe::ObserveDemo,
  slots::{Profile, SlotContainer},
};

pub(crate) struct PatternGallery {
  composition: Entity<Counter>,
  events: Entity<EventDemo>,
  global: Entity<GlobalDemo>,
  slots: Entity<SlotContainer>,
  observe: Entity<ObserveDemo>,
}

impl PatternGallery {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    // 两种不同的 Render 类型在装配层转成 AnyView，容器无需知道它们的类型。
    let profile = cx.new(|_| Profile);
    let slot_counter = cx.new(|_| Counter::default());
    let slots = cx.new(|_| SlotContainer::new(vec![profile.into(), slot_counter.into()]));
    Self {
      composition: cx.new(|_| Counter::default()),
      events: cx.new(EventDemo::new),
      global: cx.new(GlobalDemo::new),
      slots,
      observe: cx.new(ObserveDemo::new),
    }
  }
}

fn section(
  title: &'static str,
  description: &'static str,
  view: impl IntoElement,
) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .p_4()
    .border_1()
    .rounded_md()
    .child(div().text_lg().child(title))
    .child(div().text_sm().child(description))
    .child(view)
}

impl Render for PatternGallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("pattern-gallery")
      .size_full()
      .overflow_y_scroll()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        div()
          .flex()
          .flex_col()
          .gap_4()
          .p_4()
          .child(div().text_xl().child("GPUI Entity Patterns"))
          .child(section(
            "0. Composition",
            "Parent owns a concrete Entity<Counter>.",
            self.composition.clone(),
          ))
          .child(section(
            "1. Publish / Subscribe",
            "The child emits CountUpdated; the parent subscribes to it.",
            self.events.clone(),
          ))
          .child(section(
            "2. Global Context",
            "Two independent readers observe the same shared state.",
            self.global.clone(),
          ))
          .child(section(
            "3. Slot / AnyView",
            "The caller injects a profile and an interactive counter.",
            self.slots.clone(),
          ))
          .child(section(
            "4. Observe",
            "Source notify triggers the derived view to read and redraw.",
            self.observe.clone(),
          )),
      )
  }
}
