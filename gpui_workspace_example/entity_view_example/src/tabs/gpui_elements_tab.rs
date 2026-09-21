use gpui_kit::{
  AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
  Render, ScrollHandle, Styled, TestSupportExt, Window,
  component::{ActiveTheme, Selectable, button::Button},
  div, point, px,
};

use super::ComponentPage;

#[path = "gpui_elements_tab/anchored.rs"]
mod example_anchored;
#[path = "gpui_elements_tab/animation.rs"]
mod example_animation;
#[path = "gpui_elements_tab/canvas.rs"]
mod example_canvas;
#[path = "gpui_elements_tab/container_query.rs"]
mod example_container_query;
#[path = "gpui_elements_tab/deferred.rs"]
mod example_deferred;
#[path = "gpui_elements_tab/div.rs"]
mod example_div;
#[path = "gpui_elements_tab/drawable.rs"]
mod example_drawable;
#[path = "gpui_elements_tab/empty.rs"]
mod example_empty;
#[path = "gpui_elements_tab/image_cache.rs"]
mod example_image_cache;
#[path = "gpui_elements_tab/img.rs"]
mod example_img;
#[path = "gpui_elements_tab/interactive_text.rs"]
mod example_interactive_text;
#[path = "gpui_elements_tab/list.rs"]
mod example_list;
#[path = "gpui_elements_tab/quad.rs"]
mod example_quad;
#[path = "gpui_elements_tab/spring.rs"]
mod example_spring;
#[path = "gpui_elements_tab/stateful.rs"]
mod example_stateful;
#[path = "gpui_elements_tab/surface.rs"]
mod example_surface;
#[path = "gpui_elements_tab/svg.rs"]
mod example_svg;
#[path = "gpui_elements_tab/text.rs"]
mod example_text;

#[path = "gpui_elements_tab/window.rs"]
mod example_window;

struct Example {
  name: &'static str,
  constructor: &'static str,
  description: &'static str,
  reference: &'static str,
  source: &'static str,
  render: fn(&mut Window, &mut App) -> AnyElement,
}

const EXAMPLES: &[Example] = &[
  Example {
    name: "Div",
    constructor: "div()",
    description: "容器负责布局、样式和事件，可扩展滚动、焦点、拖放。下方是可点击的计数按钮。",
    reference: "hello_world.rs / drag_drop.rs",
    source: include_str!("gpui_elements_tab/div.rs"),
    render: example_div::render,
  },
  Example {
    name: "Text",
    constructor: "text!() / Text::new",
    description: "Text 是普通排版文本；富文本、高亮与装饰使用 StyledText。text! \
                  为文本生成可访问的稳定 ID。示例同时演示普通文本与局部高亮。",
    reference: "text.rs / elements/text.rs",
    source: include_str!("gpui_elements_tab/text.rs"),
    render: example_text::render,
  },
  Example {
    name: "InteractiveText",
    constructor: "InteractiveText::new",
    description: "为 StyledText 的指定 UTF-8 \
                  字节范围添加点击行为；点击蓝色英文文本更新计数，不会打开外部网站。",
    reference: "elements/text.rs",
    source: include_str!("gpui_elements_tab/interactive_text.rs"),
    render: example_interactive_text::render,
  },
  Example {
    name: "Img",
    constructor: "img()",
    description: "图片元素支持 object_fit 控制包含或裁切。两个预览使用同一张项目内置 \
                  PNG，不需要网络。",
    reference: "image/image.rs / image_loading.rs",
    source: include_str!("gpui_elements_tab/img.rs"),
    render: example_img::render,
  },
  Example {
    name: "Svg",
    constructor: "svg()",
    description: "支持 path / external_path / data；SVG 解析由 GPUI 内部完成。这里用同一份内嵌 \
                  SVG 展示不同大小和颜色，无需依赖额外 AssetSource。",
    reference: "svg/svg.rs / elements/svg.rs",
    source: include_str!("gpui_elements_tab/svg.rs"),
    render: example_svg::render,
  },
  Example {
    name: "Canvas",
    constructor: "canvas(prepaint, paint)",
    description: "prepaint 阶段拿到布局 bounds 并准备数据；paint 阶段使用 Window \
                  绘制。适合路径、图表和装饰，免去实现完整 Element。",
    reference: "painting.rs",
    source: include_str!("gpui_elements_tab/canvas.rs"),
    render: example_canvas::render,
  },
  Example {
    name: "List",
    constructor: "list() + ListState",
    description: "虚拟化可变高度列表，仅构建需要的行；ListState \
                  管理滚动和锚定。选中逻辑需自行添加；等高列表还可用 uniform_list()。",
    reference: "list_example.rs / uniform_list.rs",
    source: include_str!("gpui_elements_tab/list.rs"),
    render: example_list::render,
  },
  Example {
    name: "ContainerQuery",
    constructor: "container_query()",
    description: "根据布局分配的容器尺寸生成子内容。子内容不参与决定容器尺寸，因此需设置宽高。\
                  点击按钮比较窄容器与宽容器。",
    reference: "elements/container_query.rs",
    source: include_str!("gpui_elements_tab/container_query.rs"),
    render: example_container_query::render,
  },
  Example {
    name: "Anchored",
    constructor: "anchored()",
    description: "按局部或窗口坐标定位，并通过 snap_to_window \
                  避让窗口边界。它负责摆放，显示与关闭状态仍由业务代码管理。",
    reference: "anchor.rs",
    source: include_str!("gpui_elements_tab/anchored.rs"),
    render: example_anchored::render,
  },
  Example {
    name: "Deferred",
    constructor: "deferred()",
    description: "把子元素延迟到祖先之后绘制；priority \
                  决定多个延迟元素的顺序。切换按钮比较普通顺序与延迟绘制下的遮挡关系。",
    reference: "anchor.rs / elements/deferred.rs",
    source: include_str!("gpui_elements_tab/deferred.rs"),
    render: example_deferred::render,
  },
  Example {
    name: "Surface",
    constructor: "surface(source)（macOS）",
    description: "当前版本是原生像素缓冲显示元素：macOS 接收 \
                  CVPixelBuffer；并非通用独立绘制批次容器。下面给出已编译的完整接入函数，\
                  实际画面需要调用方提供视频或相机缓冲。",
    reference: "elements/surface.rs",
    source: include_str!("gpui_elements_tab/surface.rs"),
    render: example_surface::render,
  },
  Example {
    name: "Empty",
    constructor: "Empty",
    description: "当前版本是单元结构体，直接使用 Empty，没有 \
                  Empty::new()；不渲染且不占空间。固定尺寸占位应使用 div。",
    reference: "element.rs",
    source: include_str!("gpui_elements_tab/empty.rs"),
    render: example_empty::render,
  },
  Example {
    name: "Stateful<Div>",
    constructor: "div().id(...)",
    description: "为元素赋予稳定身份并跨帧关联交互状态；业务数据仍放在 Entity / View。示例保留 \
                  FocusHandle，点击聚焦、悬停变色。",
    reference: "focus_visible.rs / elements/div.rs",
    source: include_str!("gpui_elements_tab/stateful.rs"),
    render: example_stateful::render,
  },
  Example {
    name: "AnimationElement",
    constructor: "element.with_animation(...)",
    description: "使用 Animation、持续时间和 easing \
                  将时间进度映射到样式；点击重播。当前版本会遵循系统减少动态效果设置。",
    reference: "animation.rs / elements/animation.rs",
    source: include_str!("gpui_elements_tab/animation.rs"),
    render: example_animation::render,
  },
  Example {
    name: "SpringAnimationElement",
    constructor: "element.with_spring(...)",
    description: "用 SpringAnimation 驱动物理弹簧。稳定 ID \
                  保留位置和速度，切换目标时自然过渡；不同于固定时长的 Animation。",
    reference: "animation.rs / spring.rs",
    source: include_str!("gpui_elements_tab/spring.rs"),
    render: example_spring::render,
  },
  Example {
    name: "Quad / PaintQuad",
    constructor: "quad() → PaintQuad",
    description: "Quad 是场景内部图元；公开 quad() 返回 PaintQuad，并由 window.paint_quad \
                  提交，不能作为 child。示例在 canvas 内绘制带圆角和描边的矩形。",
    reference: "painting.rs / scene.rs / window.rs",
    source: include_str!("gpui_elements_tab/quad.rs"),
    render: example_quad::render,
  },
  Example {
    name: "ImageCacheElement",
    constructor: "image_cache(retain_all(id))",
    description: "为子树指定图片缓存，相同 Resource 的 Img 共享加载结果；稳定 ID \
                  保留缓存。retain_all 不做 LRU 淘汰，长期大量图片应选择适当缓存策略。",
    reference: "elements/image_cache.rs",
    source: include_str!("gpui_elements_tab/image_cache.rs"),
    render: example_image_cache::render,
  },
  Example {
    name: "Drawable<E>",
    constructor: "element.into_any_element()",
    description: "Drawable 包装 Element 的绘制生命周期，但 Drawable::new 是内部方法。公开用法通过 \
                  AnyElement 依次 layout_as_root、prepaint_at、paint；示例展示完整手动绘制流程。",
    reference: "element.rs",
    source: include_str!("gpui_elements_tab/drawable.rs"),
    render: example_drawable::render,
  },
  Example {
    name: "Window",
    constructor: "cx.open_window(WindowOptions, build_root)",
    description: "Window 是独立的原生窗口，不是 child \
                  元素。点击按钮创建窗口，演示窗口尺寸、标题、Root \
                  视图、独立状态和关闭；可重复创建多个窗口。",
    reference: "window.rs / window_positioning.rs / on_window_close_quit.rs",
    source: include_str!("gpui_elements_tab/window.rs"),
    render: example_window::render,
  },
];

#[derive(Default)]
pub struct GpuiElementsTab {
  selected: usize,
  code_scroll: ScrollHandle,
}

impl ComponentPage for GpuiElementsTab {
  fn title() -> &'static str {
    "GPUI 元素"
  }
  fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self::default())
  }
}

impl Render for GpuiElementsTab {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let example = &EXAMPLES[self.selected];
    let preview = (example.render)(window, cx);
    div()
      .flex()
      .flex_col()
      .gap_4()
      .w_full()
      .min_w_0()
      .child(format!(
        "{} 项 GPUI 元素、绘制工具与窗口示例。点击名称切换完整源码和对应效果；源码直接参与编译，\
         API 以本项目当前锁定的依赖版本为准。",
        EXAMPLES.len(),
      ))
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_2()
          .children(EXAMPLES.iter().enumerate().map(|(index, item)| {
            Button::new(("gpui-element-select", index))
              .label(format!("{:02} · {}", index + 1, item.name))
              .selected(self.selected == index)
              .on_click(cx.listener(move |this, _, _, cx| {
                if this.selected != index {
                  this.selected = index;
                  this.code_scroll.set_offset(point(px(0.), px(0.)));
                  cx.notify();
                }
              }))
          })),
      )
      .child(
        div()
          .text_xl()
          .child(format!("{:02} · {}", self.selected + 1, example.name)),
      )
      .child(
        div()
          .text_sm()
          .child(format!("构造方式：{}", example.constructor)),
      )
      .child(example.description)
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child(format!(
            "本地参考：zed/crates/gpui/examples/ 或 src/ 下的 {}",
            example.reference
          )),
      )
      .child(
        div()
          .text_sm()
          .child("完整代码（包含 imports 和 render 入口；window/cx 由 GPUI 提供）"),
      )
      .child(
        div()
          .id("gpui-element-code-viewport")
          .test_support()
          .flex()
          .w_full()
          .h(px(240.))
          .flex_shrink_0()
          .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
          .child(
            crate::scroll_panel::ScrollPanel::new("gpui-element-code", &self.code_scroll).child(
              div()
                .p_3()
                .bg(cx.theme().muted)
                .text_sm()
                .font_family("monospace")
                .child(example.source),
            ),
          ),
      )
      .child(div().text_lg().child(if self.selected == 10 {
        "Surface 接入说明"
      } else {
        "对应效果"
      }))
      .child(
        div()
          .id("gpui-element-preview")
          .test_support()
          .w_full()
          .min_h(px(80.))
          .p_4()
          .border_1()
          .border_color(cx.theme().border)
          .rounded_lg()
          .child(preview),
      )
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{
    AppContext, ScrollDelta, TestAppContext, component::Root, point, px, size, test::TestWindowExt,
  };

  use super::{EXAMPLES, GpuiElementsTab};

  #[gpui_kit::test]
  fn all_element_examples_render_and_switch(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx.new(|_| GpuiElementsTab::default());
    let window = cx.open_window(size(px(1200.), px(1400.)), |window, cx| {
      Root::new(view.clone(), window, cx)
    });
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      assert_eq!(EXAMPLES.len(), 19);
      window.click("element-div-button", cx);
      assert_eq!(
        window.find("element-div-button").label(),
        Some("div 点击次数：1")
      );
      for (index, example) in EXAMPLES.iter().enumerate() {
        window.click(("gpui-element-select", index), cx);
        assert_eq!(view.read(cx).selected, index, "{}", example.name);
        assert!(
          window.find("gpui-element-preview").visible(),
          "{}",
          example.name
        );
        assert_eq!(view.read(cx).code_scroll.offset().y, px(0.));
        window.scroll(
          "gpui-element-code-viewport",
          ScrollDelta::Pixels(point(px(0.), px(-80.))),
          cx,
        );
        assert!(
          view.read(cx).code_scroll.offset().y < px(0.),
          "{} code scroll",
          example.name
        );
        let action = match index {
          2 => Some("element-text-link"),
          7 => Some("element-query-toggle"),
          8 => Some("element-anchor-toggle"),
          9 => Some("element-deferred-toggle"),
          11 => Some("element-empty-toggle"),
          12 => Some("element-stateful"),
          13 => Some("element-animation-replay"),
          14 => Some("element-spring-toggle"),
          _ => None,
        };
        if let Some(id) = action {
          window.click(id, cx);
        }
      }
    })
    .unwrap();
  }

  #[gpui_kit::test]
  fn window_example_opens_independent_windows_and_closes_only_itself(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let view = cx.new(|_| GpuiElementsTab::default());
    let main: gpui_kit::AnyWindowHandle = cx
      .open_window(size(px(1200.), px(1400.)), |window, cx| {
        Root::new(view, window, cx)
      })
      .into();
    let index = EXAMPLES
      .iter()
      .position(|example| example.name == "Window")
      .unwrap();
    cx.update_window(main, |_, window, cx| {
      window.render_frame(cx);
      window.click(("gpui-element-select", index), cx);
      window.click("element-open-window", cx);
    })
    .unwrap();
    let windows = cx.windows();
    assert_eq!(windows.len(), 2);
    let first = *windows
      .iter()
      .find(|handle| handle.window_id() != main.window_id())
      .unwrap();
    cx.update_window(first, |_, window, cx| {
      window.render_frame(cx);
      window.click("new-window-counter", cx);
      assert_eq!(
        window.find("new-window-counter").label(),
        Some("独立计数：1")
      );
    })
    .unwrap();
    cx.update_window(main, |_, window, cx| {
      window.click("element-open-window", cx)
    })
    .unwrap();
    let windows = cx.windows();
    assert_eq!(windows.len(), 3);
    let second = *windows
      .iter()
      .find(|handle| {
        handle.window_id() != main.window_id() && handle.window_id() != first.window_id()
      })
      .unwrap();
    cx.update_window(second, |_, window, cx| {
      window.render_frame(cx);
      assert_eq!(
        window.find("new-window-counter").label(),
        Some("独立计数：0")
      );
      window.click("new-window-close", cx);
    })
    .unwrap();
    assert_eq!(cx.windows().len(), 2);
    cx.update_window(first, |_, window, cx| {
      window.render_frame(cx);
      assert_eq!(
        window.find("new-window-counter").label(),
        Some("独立计数：1")
      );
      window.click("new-window-close", cx);
    })
    .unwrap();
    assert_eq!(cx.windows().len(), 1);
    assert_eq!(cx.windows()[0].window_id(), main.window_id());
    cx.update_window(main, |_, window, cx| {
      window.render_frame(cx);
      assert!(window.find("element-open-window").visible());
    })
    .unwrap();
  }
}
