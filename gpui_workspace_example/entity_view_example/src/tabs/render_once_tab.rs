use gpui_kit::{
  component::{ActiveTheme, StyledExt as _, button::Button, v_flex},
  prelude::FluentBuilder as _,
  *,
};

use super::section;

/// 配置在构建阶段传入；持久状态由父 Tab 保存。
#[derive(IntoElement)]
struct DemoCard {
  title: SharedString,
  description: SharedString,
  accented: bool,
}

impl DemoCard {
  fn new(title: impl Into<SharedString>) -> Self {
    Self {
      title: title.into(),
      description: "".into(),
      accented: false,
    }
  }

  fn description(mut self, description: impl Into<SharedString>) -> Self {
    self.description = description.into();
    self
  }

  fn accented(mut self, accented: bool) -> Self {
    self.accented = accented;
    self
  }
}

impl RenderOnce for DemoCard {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    v_flex()
      .w_full()
      .gap_2()
      .p_4()
      .rounded(cx.theme().radius_lg)
      .border_1()
      .border_color(cx.theme().border)
      .bg(cx.theme().muted)
      .when(self.accented, |card| card.border_color(cx.theme().primary))
      .child(div().font_semibold().child(self.title))
      .child(self.description)
  }
}

pub struct RenderOnceTab {
  accented: bool,
}

impl super::ComponentPage for RenderOnceTab {
  fn title() -> &'static str {
    "RenderOnce · 链式调用"
  }

  fn new_view(_: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|_| Self { accented: false })
  }
}

impl Render for RenderOnceTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .w_full()
      .gap_4()
      .child(
        section("RenderOnce 组件")
          .description("点击按钮修改父 Tab 的状态，观察新建的卡片实例使用新的配置。")
          .child(
            DemoCard::new("链式配置的卡片")
              .description("标题和说明通过 builder 方法传入，强调边框由父 Tab 控制。")
              .accented(self.accented),
          )
          .child(
            Button::new("render-once-toggle")
              .label(if self.accented {
                "取消强调"
              } else {
                "强调边框"
              })
              .on_click(cx.listener(|this, _, _, cx| {
                this.accented = !this.accented;
                cx.notify();
              })),
          ),
      )
      .child(
        section("链式调用：接收 self，返回 Self")
          .child("DemoCard::new(\"标题\").description(\"说明\").accented(true)")
          .child("1. new 创建组件配置；description 接收配置的所有权，修改字段后返回自身。")
          .child("2. accented 接收上一步返回的值，继续配置。中间值按所有权传递，不需要 clone。")
          .child("3. child 接收最终组件；#[derive(IntoElement)] 将 RenderOnce 组件接入元素树。")
          .child("fn accented(mut self, accented: bool) -> Self { self.accented = accented; self }")
          .child(
            "等价展开：let card = DemoCard::new(\"标题\"); let card = card.description(\"说明\"); \
             let card = card.accented(true);",
          )
          .child(
            "同一字段重复设置时，以最后一次为准：accented(true).accented(false) \
             最终关闭强调。链式调用本身只是普通 Rust 方法调用，不会立即绘制。",
          ),
      )
      .child(
        section("Render 与 RenderOnce 的分工")
          .child(
            "父 Tab：Render::render(&mut self, ..., &mut Context<Self>)，Entity 保存 accented \
             状态，事件通过 cx.notify() 请求更新。",
          )
          .child(
            "卡片：RenderOnce::render(self, ..., &mut App)，消费当前配置并返回元素树；这里可以将 \
             title、description 直接移入子元素。",
          )
          .child(
            "Once 指当前组件实例被消费一次，不是页面永远只渲染一次。父 Tab 每次 render \
             都会创建新的 DemoCard。",
          )
          .child(
            "本例的 DemoCard 不保存跨帧状态。需要持久交互状态时，可像这里一样由父 Entity \
             持有，再通过链式方法传入。",
          )
          .child(
            "卡片内部的 .p_4().border_1().child(...) 也使用链式组合；.when(condition, |card| ...) \
             则按条件继续配置元素。",
          ),
      )
  }
}
