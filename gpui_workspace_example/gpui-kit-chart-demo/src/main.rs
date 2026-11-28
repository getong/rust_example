//! gpui-kit Chart 图表组件演示 / gpui-kit Chart demo.
//!
//! 参考 gpui-kit Chart 模块（`gpui_kit::component::chart`）：
//! LineChart、BarChart、AreaChart、PieChart、RadarChart、CandlestickChart、SankeyChart
//! 七种图表的声明式 API、动画与主题适配。
//!
//! 分模块：每种图表一个 `features::*` 模块；
//! 分功能：每个模块内按特性拆分演示卡片，并受同名 Cargo feature 门控。
//!
//! 运行：`cargo run -p gpui-kit-chart-demo`
//! 只编译部分图表：`cargo run -p gpui-kit-chart-demo --no-default-features --features line,bar`

use gpui_kit::{AppContext, SharedString, TitlebarOptions, WindowOptions, component::Root};
use gpui_kit_chart_demo::app::ChartDemo;

fn main() {
  gpui_kit::application().run(move |cx| {
    // 使用任何 GPUI Component 功能前必须先 init。
    gpui_kit::init(cx);

    cx.spawn(async move |cx| {
      cx.open_window(
        WindowOptions {
          titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("gpui-kit Chart 演示")),
            ..Default::default()
          }),
          ..Default::default()
        },
        |window, cx| {
          let view = cx.new(|_| ChartDemo::new());
          // 窗口第一层应该是 Root。
          cx.new(|cx| Root::new(view, window, cx))
        },
      )
      .expect("Failed to open window");
    })
    .detach();
  });
}
