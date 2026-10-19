use yew::prelude::*;

pub struct Home;

impl Component for Home {
  type Message = ();
  type Properties = ();

  fn create(_: &Context<Self>) -> Self {
    Self
  }

  fn update(&mut self, _: &Context<Self>, _: Self::Message) -> bool {
    false
  }

  fn view(&self, _: &Context<Self>) -> Html {
    let home_cls = "home";

    html! {
        <div class={classes!(home_cls)}>
           <h1>{ "Rust + WebAssembly" }</h1>
           <h4>{ "使用 yew 构建 WebAssembly 标准的 web 前端" }</h4>
        </div>
    }
  }
}
