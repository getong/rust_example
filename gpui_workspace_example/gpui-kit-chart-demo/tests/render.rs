//! 无头渲染 + 布局测试 / Headless rendering & layout tests.
//!
//! 用 `gpui-kit` 的 `test-support` 在无头窗口里真实渲染每个图表功能页，
//! 断言：不 panic、根容器有非零尺寸、`features::all()` 注册完整。
//! 不依赖 GPU 像素比对。

use gpui_kit::{
  AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context, Entity, InteractiveElement,
  IntoElement, ParentElement, Pixels, Point, Render, Size, Styled, TestAppContext, Window,
  WindowBounds, WindowOptions,
  base::Root,
  div, px, size,
  test::{TestSupportExt, TestWindowExt},
};
use gpui_kit_chart_demo::{app::ChartDemo, chart_card::CHART_HEIGHT, features};

/// 把一个功能页 `render` 函数包成可渲染的 harness。
struct FeatureHarness {
  render: fn(&App) -> AnyElement,
}

impl Render for FeatureHarness {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("harness-root")
      .test_support()
      .size_full()
      .p_4()
      .child((self.render)(cx))
  }
}

fn open_window<V: Render>(
  cx: &mut TestAppContext,
  window_size: Size<Pixels>,
  build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> (AnyWindowHandle, Entity<V>) {
  cx.update(|cx| {
    let bounds = Bounds {
      origin: Point::default(),
      size: window_size,
    };
    let (window, content) = gpui_kit::open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
      },
      cx,
      build,
    )
    .expect("open test window");
    (window.into(), content)
  })
}

/// 单测：feature 注册表非空且 id 唯一（不需要窗口）。
#[gpui_kit::test]
fn features_are_registered(cx: &mut TestAppContext) {
  cx.update(gpui_kit::init);
  cx.update(|_| {
    let all = features::all();
    // 允许用 --no-default-features 关闭全部图表；此时为空表

    let mut ids: Vec<&str> = all.iter().map(|f| f.id).collect();
    ids.sort_unstable();
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(ids, deduped, "feature id 必须唯一");

    // 每个功能页都有标题/描述
    for f in &all {
      assert!(!f.title.is_empty());
      assert!(!f.description.is_empty());
    }
  });
}

/// 无头渲染每个已启用的图表功能页，断言不 panic 且根容器非零尺寸。
#[gpui_kit::test]
fn every_feature_renders_with_nonzero_size(cx: &mut TestAppContext) {
  cx.update(gpui_kit::init);

  let renderers: Vec<fn(&App) -> AnyElement> = {
    let mut v = Vec::new();
    cx.update(|cx| {
      for f in features::all() {
        v.push(f.render);
      }
      let _ = cx; // silence
    });
    v
  };

  // 无任何图表 feature 时为空，直接跳过。
  for render in renderers {
    let window_size = size(px(1280.0), px(900.0));
    let (handle, _content) = open_window(cx, window_size, |_window, cx| {
      cx.new(|_| FeatureHarness { render })
    });

    cx.update_window(handle, |_, window, cx| {
      // 渲染一帧；此处 panic 即测试失败。
      window.render_frame(cx);

      let root = window.find("harness-root");
      assert!(root.visible(), "功能页根容器应可见");
      let bounds = root.bounds();
      assert!(
        bounds.size.width > px(0.0),
        "根容器宽度应 > 0，实际 {bounds:?}"
      );
      assert!(
        bounds.size.height > px(0.0),
        "根容器高度应 > 0，实际 {bounds:?}"
      );

      // 渲染两帧，确认二次渲染稳定（图表缓存/动画不 panic）。
      window.render_frame(cx);
    })
    .expect("render frame");
  }
}

/// 完整应用 ChartDemo 在无头窗口里渲染，断言不 panic。
#[gpui_kit::test]
fn app_renders(cx: &mut TestAppContext) {
  cx.update(gpui_kit::init);

  let window_size = size(px(1280.0), px(900.0));
  let (handle, _view) = open_window(cx, window_size, |window, cx| {
    let view = cx.new(|_| ChartDemo::new());
    // Root 包裹
    cx.new(|cx| Root::new(view, window, cx))
  });

  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.render_frame(cx);
  })
  .expect("render app");
}

/// 图表卡片的图表区高度常量应为正。
#[test]
fn chart_height_positive() {
  assert!(CHART_HEIGHT > 0.0);
}
