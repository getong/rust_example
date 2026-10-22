use gpui_kit::{
  component::{
    Root,
    button::{Button, ButtonVariants},
  },
  *,
};
use taffy::{
  TaffyTree,
  geometry::Size,
  prelude::{
    AvailableSpace, Dimension, FlexDirection, LengthPercentage, LengthPercentageAuto, Style,
  },
};

const MIN_CONTENT_WIDTH: f32 = 220.0;
const MIN_CONTENT_HEIGHT: f32 = 180.0;

#[derive(Clone, Copy, Debug, Default)]
struct Rect {
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

impl Rect {
  fn from_layout(layout: &taffy::Layout) -> Self {
    Self {
      x: layout.location.x,
      y: layout.location.y,
      width: layout.size.width,
      height: layout.size.height,
    }
  }
}

#[derive(Clone, Copy, Debug, Default)]
struct LayoutRects {
  sidebar: Rect,
  header: Rect,
  content: Rect,
  card_a: Rect,
  card_b: Rect,
  card_c: Rect,
}

/// Taffy owns the outer layout tree. GPUI Kit owns the component tree painted inside it.
fn compute_layout(viewport_width: f32, viewport_height: f32) -> LayoutRects {
  let width = viewport_width.max(MIN_CONTENT_WIDTH);
  let height = viewport_height.max(MIN_CONTENT_HEIGHT);
  let mut tree = TaffyTree::<()>::new();

  let card_style = || Style {
    flex_grow: 1.0,
    min_size: Size {
      width: LengthPercentageAuto::length(0.0),
      height: LengthPercentageAuto::length(0.0),
    },
    ..Default::default()
  };
  let card_a = tree.new_leaf(card_style()).expect("create card A");
  let card_b = tree.new_leaf(card_style()).expect("create card B");
  let card_c = tree.new_leaf(card_style()).expect("create card C");

  let cards = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Row,
        gap: Size {
          width: LengthPercentage::length(12.0),
          height: LengthPercentage::length(0.0),
        },
        flex_grow: 1.0,
        ..Default::default()
      },
      &[card_a, card_b, card_c],
    )
    .expect("create cards container");

  let header = tree
    .new_leaf(Style {
      size: Size {
        width: Dimension::auto(),
        height: Dimension::length(76.0),
      },
      ..Default::default()
    })
    .expect("create header");

  let content = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Column,
        gap: Size {
          width: LengthPercentage::length(0.0),
          height: LengthPercentage::length(16.0),
        },
        flex_grow: 1.0,
        min_size: Size {
          width: LengthPercentageAuto::length(0.0),
          height: LengthPercentageAuto::length(0.0),
        },
        ..Default::default()
      },
      &[header, cards],
    )
    .expect("create content container");

  let sidebar = tree
    .new_leaf(Style {
      size: Size {
        width: Dimension::length(176.0),
        height: Dimension::auto(),
      },
      ..Default::default()
    })
    .expect("create sidebar");

  let root = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Row,
        gap: Size {
          width: LengthPercentage::length(24.0),
          height: LengthPercentage::length(0.0),
        },
        size: Size {
          width: Dimension::length(width),
          height: Dimension::length(height),
        },
        padding: taffy::Rect {
          left: LengthPercentage::length(24.0),
          right: LengthPercentage::length(24.0),
          top: LengthPercentage::length(24.0),
          bottom: LengthPercentage::length(24.0),
        },
        ..Default::default()
      },
      &[sidebar, content],
    )
    .expect("create root");

  tree
    .compute_layout(
      root,
      Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::Definite(height),
      },
    )
    .expect("compute Taffy layout");

  LayoutRects {
    sidebar: Rect::from_layout(tree.layout(sidebar).expect("sidebar layout")),
    header: Rect::from_layout(tree.layout(header).expect("header layout")),
    content: Rect::from_layout(tree.layout(content).expect("content layout")),
    card_a: Rect::from_layout(tree.layout(card_a).expect("card A layout")),
    card_b: Rect::from_layout(tree.layout(card_b).expect("card B layout")),
    card_c: Rect::from_layout(tree.layout(card_c).expect("card C layout")),
  }
}

fn positioned(rect: Rect) -> Div {
  div()
    .absolute()
    .left(px(rect.x))
    .top(px(rect.y))
    .w(px(rect.width))
    .h(px(rect.height))
}

struct TaffyKitDemo {
  count: u64,
}

impl Render for TaffyKitDemo {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let viewport = window.viewport_size();
    let rects = compute_layout(f32::from(viewport.width), f32::from(viewport.height));

    div()
      .relative()
      .size_full()
      .bg(rgb(0x111827))
      .text_color(rgb(0xf3f4f6))
      .child(
        positioned(rects.sidebar)
          .p_4()
          .rounded_md()
          .bg(rgb(0x1f2937))
          .child(
            div()
              .text_lg()
              .font_weight(FontWeight::SEMIBOLD)
              .child("Taffy + GPUI Kit"),
          )
          .child(
            div()
              .mt_3()
              .text_sm()
              .text_color(rgb(0x9ca3af))
              .child("Taffy computes the outer boxes."),
          )
          .child(
            div()
              .mt_2()
              .text_sm()
              .text_color(rgb(0x9ca3af))
              .child("Kit renders the components."),
          ),
      )
      .child(
        positioned(rects.header)
          .p_4()
          .rounded_md()
          .bg(rgb(0x2563eb))
          .child(div().text_2xl().child("Layout boundary"))
          .child(
            div()
              .mt_2()
              .text_sm()
              .text_color(rgb(0xdbeafe))
              .child("Resize the window to recompute the Taffy tree."),
          ),
      )
      .child(
        positioned(rects.card_a)
          .p_4()
          .rounded_md()
          .bg(rgb(0x047857))
          .child(div().text_lg().child("Card A"))
          .child(
            div()
              .mt_2()
              .text_sm()
              .child("A GPUI Kit button inside a Taffy box."),
          )
          .child(
            Button::new("increment")
              .mt_4()
              .primary()
              .label(format!("Count: {}", self.count))
              .on_click(cx.listener(|this, _, _, cx| {
                this.count = this.count.saturating_add(1);
                cx.notify();
              })),
          ),
      )
      .child(
        positioned(rects.card_b)
          .p_4()
          .rounded_md()
          .bg(rgb(0x0f766e))
          .child(div().text_lg().child("Card B"))
          .child(
            div()
              .mt_2()
              .text_sm()
              .child("The card position comes from Taffy."),
          ),
      )
      .child(
        positioned(rects.card_c)
          .p_4()
          .rounded_md()
          .bg(rgb(0x115e59))
          .child(div().text_lg().child("Card C"))
          .child(
            div()
              .mt_2()
              .text_sm()
              .child("The content is still native GPUI."),
          ),
      )
      .child(
        positioned(rects.content)
          .top(px(rects.content.y + rects.content.height - 24.0))
          .h(px(20.0))
          .text_sm()
          .text_color(rgb(0x9ca3af))
          .child("Taffy -> rectangles -> GPUI Kit components"),
      )
  }
}

fn main() {
  gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(760.0), px(460.0)),
        cx,
      ))),
      window_min_size: Some(size(px(400.0), px(320.0))),
      titlebar: Some(TitlebarOptions {
        title: Some("Taffy + GPUI Kit".into()),
        ..Default::default()
      }),
      ..Default::default()
    };
    cx.open_window(options, |window, cx| {
      let view = cx.new(|_| TaffyKitDemo { count: 0 });
      cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("failed to open the application window");
    cx.activate(true);
  });
}
