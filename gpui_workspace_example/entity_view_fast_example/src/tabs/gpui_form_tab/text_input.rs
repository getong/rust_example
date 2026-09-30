//! 本页的字符串输入适配器；端口解析由表单提交时处理。
use component_shape_gpui::{GpuiComponentValueBinding, component_shape};
use gpui_form::runtime::shape::{DirectValueStorage, GpuiFormComponentShapePolicy, ValueChange};
use gpui_kit::{
  Context, Window,
  component::input::{InputEvent, InputState},
};

component_shape! {
  pub struct FormInput {
    state = InputState;
    new = |window, cx| InputState::new(window, cx);
    component = gpui_kit::component::input::Input;
    value = String;
    field_suffix = "input";
    value_binding;

    impl GpuiComponentValueBinding<String> for FormInput {
      type Event = InputEvent;

      fn seed_value_binding_state(
        state: &mut Self::State,
        value: Option<&String>,
        window: &mut Window,
        cx: &mut Context<'_, Self::State>,
      ) {
        state.set_value(value.cloned().unwrap_or_default(), window, cx);
      }

      fn value_change(state: &Self::State, event: &Self::Event) -> ValueChange<String> {
        match event {
          InputEvent::Change if state.value().is_empty() => ValueChange::Clear,
          InputEvent::Change => ValueChange::Set(state.value().to_string()),
          _ => ValueChange::Unchanged,
        }
      }
    }
  }
}

impl GpuiFormComponentShapePolicy for FormInput {
  type ValueStoragePolicy = DirectValueStorage;
}
