//! 全局状态：编辑器与多个读者直接访问同一 Global，无须逐层传递数据。
use gpui_kit::{component::button::*, *};

#[derive(Default)]
struct SharedCount(u32);
impl Global for SharedCount {}

pub(crate) fn init(cx: &mut App) {
  cx.set_global(SharedCount::default());
}

struct Editor;
impl Render for Editor {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Button::new("global-increment")
      .label("Shared +1")
      .on_click(|_, _, cx| {
        // update_global 会通知 observe_global 的订阅者。
        cx.update_global::<SharedCount, _>(|state, _| state.0 += 1);
      })
  }
}

struct Reader {
  name: &'static str,
  _subscription: Subscription,
}
impl Reader {
  fn new(name: &'static str, cx: &mut Context<Self>) -> Self {
    let subscription = cx.observe_global::<SharedCount>(|_, cx| cx.notify());
    Self {
      name,
      _subscription: subscription,
    }
  }
}
impl Render for Reader {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div().child(format!("{}: {}", self.name, cx.global::<SharedCount>().0))
  }
}

pub(crate) struct GlobalDemo {
  editor: Entity<Editor>,
  readers: [Entity<Reader>; 2],
}
impl GlobalDemo {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    Self {
      editor: cx.new(|_| Editor),
      readers: [
        cx.new(|cx| Reader::new("Reader A", cx)),
        cx.new(|cx| Reader::new("Reader B", cx)),
      ],
    }
  }
}
impl Render for GlobalDemo {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .items_center()
      .gap_3()
      .child(self.editor.clone())
      .children(self.readers.iter().cloned())
  }
}
