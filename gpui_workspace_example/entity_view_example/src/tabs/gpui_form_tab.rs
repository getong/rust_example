//! gpui-form 负责模型与编辑状态之间的类型桥接；布局、订阅和提交由应用组织。
//! 参考 gpui-form/examples/some-lib-forms 的 Components → Fields → ValueHolder 流程。
use gpui_form::{
  GpuiForm,
  runtime::shape::{ValueChange, seed_value_binding_state, value_change},
};
use gpui_form_collection::input::Input as FormInput;
use gpui_kit::{
  component::{
    ActiveTheme,
    button::Button,
    form::{field, v_form},
    input::{Input, InputEvent},
    v_flex,
  },
  *,
};

/// 表单中的端口保留为 String，允许用户暂时输入无效文本；提交时才转换为 u16。
#[derive(Clone, Debug, PartialEq, Eq, GpuiForm)]
#[gpui_form(no_inventory)]
struct ConnectionProfile {
  #[gpui_form(component(FormInput::<String>))]
  name: String,
  #[gpui_form(component(FormInput::<String>, value(
    type = String,
    from_source = port_to_text,
    try_into_source = parse_port,
  )))]
  port: u16,
  // hidden 仍保存在 ValueHolder 中，只是不生成可见控件。
  #[gpui_form(hidden(default = String::from("local-demo")))]
  profile_id: String,
}

fn port_to_text(port: u16) -> String {
  port.to_string()
}

fn parse_port(text: String) -> Result<u16, &'static str> {
  text
    .trim()
    .parse::<u16>()
    .ok()
    .filter(|port| *port > 0)
    .ok_or("端口必须是 1–65535 的整数")
}

pub struct GpuiFormTab {
  // 以下两个类型和 Components / FormField 都由 derive 生成。
  fields: ConnectionProfileFormFields,
  draft: ConnectionProfileFormValueHolder,
  result: String,
  failed: bool,
  _subscriptions: Vec<Subscription>,
}

impl super::ComponentPage for GpuiFormTab {
  fn title() -> &'static str {
    "gpui-form"
  }

  fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
    cx.new(|cx| Self::new(window, cx))
  }
}

impl GpuiFormTab {
  fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    // Components 生成控件状态工厂；Fields 将这些 Entity 集中保存。
    let name = cx.new(|cx| ConnectionProfileFormComponents::name(window, cx));
    let port = cx.new(|cx| ConnectionProfileFormComponents::port(window, cx));
    let subscriptions = vec![
      cx.subscribe_in(&name, window, |this, state, event: &InputEvent, _, cx| {
        match value_change::<FormInput<String>, String>(state.read(cx), event) {
          ValueChange::Set(value) => this.draft.name = value,
          ValueChange::Clear => this.draft.name.clear(),
          ValueChange::Unchanged => return,
        }
        this.result = "编辑中，点击提交检查当前数据。".into();
        this.failed = false;
        cx.notify();
      }),
      cx.subscribe_in(&port, window, |this, state, event: &InputEvent, _, cx| {
        match value_change::<FormInput<String>, String>(state.read(cx), event) {
          ValueChange::Set(value) => this.draft.port = value,
          ValueChange::Clear => this.draft.port.clear(),
          ValueChange::Unchanged => return,
        }
        this.result = "编辑中，点击提交检查当前数据。".into();
        this.failed = false;
        cx.notify();
      }),
    ];
    Self {
      fields: ConnectionProfileFormFields { name, port },
      draft: ConnectionProfileFormValueHolder::default(),
      result: "输入名称和端口，或点击回填示例。提交只在本地转换数据。".into(),
      failed: false,
      _subscriptions: subscriptions,
    }
  }

  fn fill(
    &mut self,
    draft: ConnectionProfileFormValueHolder,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    // 从模型回填时同时更新 holder 和控件；只修改 holder 不会自动更新输入框。
    self.fields.name.update(cx, |state, cx| {
      seed_value_binding_state::<FormInput<String>, String>(state, Some(&draft.name), window, cx);
    });
    self.fields.port.update(cx, |state, cx| {
      seed_value_binding_state::<FormInput<String>, String>(state, Some(&draft.port), window, cx);
    });
    self.draft = draft;
    self.result = "数据已回填，可继续编辑并提交。".into();
    self.failed = false;
    cx.notify();
  }

  fn submit(&mut self, cx: &mut Context<Self>) {
    // 布局组件的 required(true) 只是视觉标记。业务校验需要显式实现。
    if self.draft.name.trim().is_empty() {
      self.failed = true;
      self.result = "name：名称不能为空（应用层校验）。".into();
    } else {
      match self.draft.clone().try_into_original() {
        Ok(profile) => {
          self.failed = false;
          self.result = format!("转换成功（未发送网络请求）：{profile:?}");
        }
        Err(error) => {
          self.failed = true;
          self.result = format!("字段 {} 转换失败：{error}", error.field_name);
        }
      }
    }
    cx.notify();
  }
}

impl Render for GpuiFormTab {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .w_full()
      .gap_4()
      .child(div().text_xl().child("Rust 模型 → 表单草稿 → 业务模型"))
      .child(
        "GpuiForm 为结构体生成控件工厂、字段实体容器、ValueHolder \
         和转换方法，减少重复代码。gpui-kit 的 field / v_form 负责可见布局。",
      )
      .child(
        v_form()
          .child(
            field()
              .label("连接名称")
              .required(true)
              .description("应用层检查非空；输入会实时同步到下方草稿。")
              .child(Input::new(&self.fields.name)),
          )
          .child(
            field()
              .label("端口")
              .required(true)
              .description("试试 abc、0 或 70000：草稿保留原文，提交时报告转换错误。")
              .child(Input::new(&self.fields.port)),
          ),
      )
      .child(
        div()
          .flex()
          .flex_wrap()
          .gap_2()
          .child(
            Button::new("gpui-form-fill")
              .label("回填示例")
              .on_click(cx.listener(|this, _, window, cx| {
                this.fill(
                  ConnectionProfileFormValueHolder::from(ConnectionProfile {
                    name: "开发服务".into(),
                    port: 8080,
                    profile_id: "profile-42".into(),
                  }),
                  window,
                  cx,
                );
              })),
          )
          .child(
            Button::new("gpui-form-submit")
              .label("提交并转换")
              .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
          )
          .child(
            Button::new("gpui-form-reset")
              .label("重置")
              .on_click(cx.listener(|this, _, window, cx| {
                this.fill(ConnectionProfileFormValueHolder::default(), window, cx);
              })),
          ),
      )
      .child(
        div()
          .p_3()
          .rounded_lg()
          .bg(cx.theme().muted)
          .child(format!("当前 ValueHolder：{:?}", self.draft)),
      )
      .child(
        div()
          .text_color(if self.failed {
            cx.theme().danger
          } else {
            cx.theme().foreground
          })
          .child(self.result.clone()),
      )
      .child(div().text_lg().child("代码中的关键步骤"))
      .children(
        [
          "1. #[derive(GpuiForm)] + component(FormInput::<String>)：声明业务字段与控件的对应关系。",
          "2. ConnectionProfileFormComponents / FormFields：创建和持有 InputState 实体。",
          "3. subscribe_in + value_change：把 Set / Clear 写入草稿；忽略 Blur 等 Unchanged 事件。",
          "4. From<ConnectionProfile> + seed_value_binding_state：加载已有数据到编辑界面。",
          "5. try_into_original：将端口文本转为 u16，转换错误携带字段名。",
          "6. hidden：profile_id 随草稿回填并提交，但不创建可见输入框。",
        ]
        .into_iter()
        .map(|line| div().text_sm().child(line)),
      )
      .child(
        "适用：设置页、编辑对话框、配置工具。更复杂的校验可接 Koruma；inventory、MCP \
         和原型代码生成属于可选扩展，本例未启用。",
      )
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{
    AppContext, TestAppContext,
    component::{Root, input::InputEvent},
    px, size,
    test::TestWindowExt,
  };

  use super::{ConnectionProfile, ConnectionProfileFormValueHolder, GpuiFormTab};

  #[gpui_kit::test]
  fn form_fill_edit_submit_and_reset(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let window = cx.open_window(size(px(1200.), px(1000.)), |window, cx| {
      let view = cx.new(|cx| GpuiFormTab::new(window, cx));
      Root::new(view, window, cx)
    });
    cx.update_window(window.into(), |root, window, cx| {
      let view = root
        .downcast::<Root>()
        .unwrap()
        .read(cx)
        .view()
        .clone()
        .downcast::<GpuiFormTab>()
        .unwrap();
      window.render_frame(cx);
      window.click("gpui-form-fill", cx);
      assert_eq!(view.read(cx).fields.port.read(cx).value().as_ref(), "8080");
      assert_eq!(view.read(cx).draft.profile_id, "profile-42");
      window.click("gpui-form-submit", cx);
      assert!(!view.read(cx).failed);
      assert!(view.read(cx).result.contains("转换成功"));

      let port = view.read(cx).fields.port.clone();
      port.update(cx, |state, cx| {
        state.set_value("abc", window, cx);
        cx.emit(InputEvent::Change);
      });
    })
    .unwrap();
    // 退出 update 以处理输入事件订阅，再验证提交读到的是当前草稿。
    cx.update_window(window.into(), |root, window, cx| {
      let view = root
        .downcast::<Root>()
        .unwrap()
        .read(cx)
        .view()
        .clone()
        .downcast::<GpuiFormTab>()
        .unwrap();
      assert_eq!(view.read(cx).draft.port, "abc");
      window.render_frame(cx);
      window.click("gpui-form-submit", cx);
      assert!(view.read(cx).failed);
      assert!(view.read(cx).result.contains("port"));
      window.click("gpui-form-reset", cx);
      assert!(view.read(cx).fields.port.read(cx).value().is_empty());
      assert!(view.read(cx).draft.port.is_empty());
      assert_eq!(view.read(cx).draft.profile_id, "local-demo");
      window.click("gpui-form-submit", cx);
      assert!(view.read(cx).result.contains("名称不能为空"));
    })
    .unwrap();
  }

  #[test]
  fn model_roundtrip_preserves_hidden_id_and_converts_port() {
    let original = ConnectionProfile {
      name: "demo".into(),
      port: 8080,
      profile_id: "id-7".into(),
    };
    let mut draft = ConnectionProfileFormValueHolder::from(original.clone());
    assert_eq!(draft.port, "8080");
    assert_eq!(draft.clone().try_into_original().unwrap(), original);
    draft.port = "443".into();
    let updated = draft.try_into_original().unwrap();
    assert_eq!(updated.port, 443);
    assert_eq!(updated.profile_id, "id-7");
  }

  #[test]
  fn invalid_port_reports_field_instead_of_silently_using_a_default() {
    for value in ["", "abc", "0", "65536", "-1"] {
      let draft = ConnectionProfileFormValueHolder {
        port: value.into(),
        ..Default::default()
      };
      assert_eq!(draft.try_into_original().unwrap_err().field_name, "port");
    }
    for value in ["1", "65535", " 8080 "] {
      let draft = ConnectionProfileFormValueHolder {
        port: value.into(),
        ..Default::default()
      };
      assert!(draft.try_into_original().is_ok());
    }
  }
}
