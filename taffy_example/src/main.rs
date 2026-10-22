use gpui::{
  App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use taffy::{
  TaffyTree,
  geometry::Size,
  prelude::{Dimension, FlexDirection, LengthPercentage, Style, TaffyMaxContent},
};

const VIEWPORT_WIDTH: f32 = 760.0;
const VIEWPORT_HEIGHT: f32 = 460.0;

#[derive(Clone, Copy)]
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

struct LayoutRects {
  sidebar: Rect,
  header: Rect,
  content: Rect,
  card_a: Rect,
  card_b: Rect,
  card_c: Rect,
}

/// Builds one small CSS-like layout tree and turns its computed boxes into
/// coordinates that GPUI can paint.
fn compute_layout() -> LayoutRects {
  let mut tree = TaffyTree::<()>::new();

  let card_style = || Style {
    flex_grow: 1.0,
    ..Default::default()
  };
  let card_a = tree.new_leaf(card_style()).unwrap();
  let card_b = tree.new_leaf(card_style()).unwrap();
  let card_c = tree.new_leaf(card_style()).unwrap();

  let cards = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Row,
        gap: taffy::geometry::Size {
          width: LengthPercentage::length(12.0),
          height: LengthPercentage::length(0.0),
        },
        flex_grow: 1.0,
        ..Default::default()
      },
      &[card_a, card_b, card_c],
    )
    .unwrap();

  let header = tree
    .new_leaf(Style {
      size: Size {
        width: Dimension::auto(),
        height: Dimension::length(76.0),
      },
      ..Default::default()
    })
    .unwrap();

  let content = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Column,
        gap: taffy::geometry::Size {
          width: LengthPercentage::length(0.0),
          height: LengthPercentage::length(16.0),
        },
        flex_grow: 1.0,
        ..Default::default()
      },
      &[header, cards],
    )
    .unwrap();

  let sidebar = tree
    .new_leaf(Style {
      size: Size {
        width: Dimension::length(176.0),
        height: Dimension::auto(),
      },
      ..Default::default()
    })
    .unwrap();

  let root = tree
    .new_with_children(
      Style {
        flex_direction: FlexDirection::Row,
        gap: taffy::geometry::Size {
          width: LengthPercentage::length(24.0),
          height: LengthPercentage::length(0.0),
        },
        size: Size {
          width: Dimension::length(VIEWPORT_WIDTH),
          height: Dimension::length(VIEWPORT_HEIGHT),
        },
        padding: taffy::geometry::Rect {
          left: LengthPercentage::length(24.0),
          right: LengthPercentage::length(24.0),
          top: LengthPercentage::length(24.0),
          bottom: LengthPercentage::length(24.0),
        },
        ..Default::default()
      },
      &[sidebar, content],
    )
    .unwrap();

  tree.compute_layout(root, Size::MAX_CONTENT).unwrap();

  LayoutRects {
    sidebar: Rect::from_layout(tree.layout(sidebar).unwrap()),
    header: Rect::from_layout(tree.layout(header).unwrap()),
    content: Rect::from_layout(tree.layout(content).unwrap()),
    card_a: Rect::from_layout(tree.layout(card_a).unwrap()),
    card_b: Rect::from_layout(tree.layout(card_b).unwrap()),
    card_c: Rect::from_layout(tree.layout(card_c).unwrap()),
  }
}

fn panel(rect: Rect, color: u32, text: &'static str) -> impl IntoElement {
  div()
    .absolute()
    .left(px(rect.x))
    .top(px(rect.y))
    .w(px(rect.width))
    .h(px(rect.height))
    .p_4()
    .rounded_md()
    .bg(rgb(color))
    .text_color(rgb(0xe5e7eb))
    .child(text)
}

struct TaffyGpuiDemo {
  rects: LayoutRects,
}

impl TaffyGpuiDemo {
  fn new() -> Self {
    Self {
      rects: compute_layout(),
    }
  }
}

impl Render for TaffyGpuiDemo {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    let r = &self.rects;
    div()
      .relative()
      .size_full()
      .bg(rgb(0x111827))
      .child(panel(r.sidebar, 0x1f2937, "Sidebar\nTaffy flex item"))
      .child(panel(r.header, 0x2563eb, "Header\ncomputed by Taffy"))
      .child(panel(r.card_a, 0x047857, "Card A"))
      .child(panel(r.card_b, 0x0f766e, "Card B"))
      .child(panel(r.card_c, 0x115e59, "Card C"))
      .child(
        div()
          .absolute()
          .left(px(r.content.x))
          .top(px(r.content.y + r.content.height - 28.0))
          .w(px(r.content.width))
          .h(px(20.0))
          .text_sm()
          .text_color(rgb(0x9ca3af))
          .child("Taffy computes boxes; GPUI paints them"),
      )
  }
}

fn main() {
  application().run(|cx: &mut App| {
    let bounds = Bounds::centered(None, size(px(VIEWPORT_WIDTH), px(VIEWPORT_HEIGHT)), cx);
    cx.open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
      },
      |_, cx| cx.new(|_| TaffyGpuiDemo::new()),
    )
    .unwrap();
    cx.activate(true);
  });
}
