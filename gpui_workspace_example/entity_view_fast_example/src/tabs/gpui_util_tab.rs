//! gpui-toolkit-gpui-util 基础工具的交互入口。
use gpui_kit::{
  component::{button::Button, v_flex},
  prelude::FluentBuilder as _,
  *,
};

use super::{gpui_util_examples as examples, section};

type DemoCase = (&'static str, &'static str, fn() -> String);

pub struct GpuiUtilTab {
  results: [String; 5],
}

impl super::ComponentPage for GpuiUtilTab {
  fn title() -> &'static str {
    "gpui-toolkit-gpui-util"
  }

  fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self {
      results: std::array::from_fn(|_| "点击按钮运行案例".into()),
    })
  }
}

impl Render for GpuiUtilTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let demos: [DemoCase; 5] = [
      (
        "ArcCow：借用与共享",
        "静态内容可借用；动态内容由 Arc 持有，clone 共享数据。此类型没有 to_mut 写时复制接口。",
        examples::sharing,
      ),
      (
        "ResultExt / TryFutureExt / maybe!",
        "错误转为 None 并交给 log 日志后端；maybe! 允许在局部块使用 ?。异步案例使用立即完成的 \
         Future。",
        examples::errors,
      ),
      (
        "defer：作用域清理",
        "持有 guard 到作用域结束自动清理；abort() 可取消。适合临时状态和资源收尾。",
        || examples::cleanup(false),
      ),
      (
        "measure：按需计时",
        "执行闭包并原样返回结果；ZED_MEASUREMENTS 首次调用时读取并缓存。",
        examples::measurement,
      ),
      (
        "post_inc / TypeId / Command",
        "返回递增前的值、按类型查找服务、跨平台构造进程命令。TypeIdHashBuilder 仅用于 TypeId 键。",
        examples::helpers,
      ),
    ];

    v_flex()
      .gap_4()
      .child(
        "这是通用 Rust 工具库，导入名为 gpui_util，不提供 UI 控件。完整用法见 docs/gpui-util.md。",
      )
      .children(
        demos
          .into_iter()
          .enumerate()
          .map(|(index, (title, description, run))| {
            section(title).description(description).child(
              v_flex()
                .w_full()
                .gap_3()
                .child(
                  Button::new(("run-util", index))
                    .label("运行案例")
                    .on_click(cx.listener(move |this, _, _, cx| {
                      this.results[index] = run();
                      cx.notify();
                    })),
                )
                .when(index == 2, |this| {
                  this.child(
                    Button::new("cancel-util-cleanup")
                      .label("运行并取消清理")
                      .on_click(cx.listener(|this, _, _, cx| {
                        this.results[2] = examples::cleanup(true);
                        cx.notify();
                      })),
                  )
                })
                .child(self.results[index].clone()),
            )
          }),
      )
  }
}
