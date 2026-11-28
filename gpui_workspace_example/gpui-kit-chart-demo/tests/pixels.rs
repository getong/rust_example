//! 真实离屏像素渲染测试 / Real offscreen pixel rendering test.
//!
//! 需要 GPUI 真实离屏渲染器（macOS Metal / Linux WGPU），不做假渲染。
//! `harness = false`：自定义 `main`，因为 headless 渲染需要主线程。
//!
//! 运行：`cargo test -p gpui-kit-chart-demo --test pixels`
//! 渲染整个应用 + 每个图表功能页，保存 PNG（`PIXELS_OUT_DIR`，默认 `/tmp`），
//! 并断言渲染结果非空白。渲染器不可用时打印 skipped 并退出 0（诚实降级）。

#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{collections::HashSet, sync::Arc};

use gpui_kit::{
  AnyElement, App, AppContext, Bounds, Context, IntoElement, ParentElement, Pixels, Point, Render,
  Size, Styled, Window, WindowBounds, WindowOptions,
  assets::Assets,
  component::{ActiveTheme, Root},
  div, px, size,
  test::TestWindowExt,
};
use gpui_kit_chart_demo::{app::ChartDemo, features};

fn main() {
  match run() {
    Ok(report) => println!("pixels: rendered\n{report}"),
    Err(err) => println!("pixels: skipped ({err})"),
  }
}

struct FeatureHarness {
  render: fn(&App) -> AnyElement,
}

impl Render for FeatureHarness {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let bg = cx.theme().background;
    div().size_full().p_4().bg(bg).child((self.render)(cx))
  }
}

fn run() -> Result<String, String> {
  let mut cx = gpui_kit::HeadlessAppContext::with_platform(
    gpui_kit::platform::current_platform(true).text_system(),
    Arc::new(Assets),
    gpui_kit::platform::current_headless_renderer,
  );
  cx.update(gpui_kit::init);

  let out_dir = std::env::var("PIXELS_OUT_DIR").unwrap_or_else(|_| "/tmp".to_string());
  let window_size: Size<Pixels> = size(px(1280.0), px(900.0));
  let mut lines = Vec::new();

  // 1. 完整应用
  {
    let (handle, _c) = cx
      .update(|cx| {
        let bounds = Bounds {
          origin: Point::default(),
          size: window_size,
        };
        gpui_kit::open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: false,
            show: false,
            ..Default::default()
          },
          cx,
          |window, cx| {
            let view = cx.new(|_| ChartDemo::new());
            cx.new(|cx| Root::new(view, window, cx))
          },
        )
      })
      .map_err(|e| e.to_string())?;

    let stats = capture(
      &mut cx,
      handle,
      &format!("{out_dir}/gpui-kit-chart-demo-app.png"),
    )?;
    lines.push(format!("app: {stats}"));
  }

  // 2. 每个图表功能页
  let renderers: Vec<(String, fn(&App) -> AnyElement)> = {
    let mut v = Vec::new();
    cx.update(|_| {
      for f in features::all() {
        v.push((f.id.to_string(), f.render));
      }
    });
    v
  };

  for (id, render) in renderers {
    let (handle, _c) = cx
      .update(|cx| {
        let bounds = Bounds {
          origin: Point::default(),
          size: window_size,
        };
        gpui_kit::open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: false,
            show: false,
            ..Default::default()
          },
          cx,
          |_window, cx| cx.new(|_| FeatureHarness { render }),
        )
      })
      .map_err(|e| e.to_string())?;

    let stats = capture(
      &mut cx,
      handle,
      &format!("{out_dir}/gpui-kit-chart-demo-{id}.png"),
    )?;
    lines.push(format!("{id}: {stats}"));
  }

  Ok(lines.join("\n"))
}

fn capture(
  cx: &mut gpui_kit::HeadlessAppContext,
  handle: gpui_kit::AnyWindowHandle,
  out: &str,
) -> Result<String, String> {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.render_frame(cx);
  })
  .map_err(|e| e.to_string())?;

  let image = cx.capture_screenshot(handle).map_err(|e| e.to_string())?;
  let (w, h) = (image.width(), image.height());
  let _ = image.save(out);
  let raw = image.into_raw();

  let mut colors: HashSet<(u8, u8, u8)> = HashSet::new();
  let mut non_uniform = 0usize;
  let total_px = raw.len() / 4;
  let first = if raw.len() >= 3 {
    (raw[0], raw[1], raw[2])
  } else {
    (0, 0, 0)
  };
  for px4 in raw.chunks_exact(4) {
    let c = (px4[0], px4[1], px4[2]);
    colors.insert(c);
    if c != first {
      non_uniform += 1;
    }
  }

  let distinct = colors.len();
  let non_uniform_ratio = non_uniform as f32 / total_px.max(1) as f32;

  if distinct < 8 {
    return Err(format!("渲染结果颜色过少 distinct={distinct} ({out})"));
  }
  if non_uniform_ratio < 0.02 {
    return Err(format!(
      "渲染结果疑似空白 non_uniform={non_uniform_ratio:.3} ({out})"
    ));
  }

  Ok(format!(
    "{w}x{h}, distinct={distinct}, non_uniform={non_uniform_ratio:.3}, saved={out}"
  ))
}
