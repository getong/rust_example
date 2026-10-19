use wasm_bindgen::prelude::*;
use yew::prelude::*;

struct Model {
  value: i64,
}

enum Msg {
  AddOne,
}

impl Component for Model {
  type Message = Msg;
  type Properties = ();
  fn create(_: &Context<Self>) -> Self {
    Self { value: 0 }
  }

  fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
    match msg {
      Msg::AddOne => self.value += 1,
    }
    true
  }

  fn view(&self, ctx: &Context<Self>) -> Html {
    html! {
        <div>
            <button onclick={ctx.link().callback(|_| Msg::AddOne)}>{ "+1" }</button>
            <p>{ self.value }</p>
        </div>
    }
  }
}

#[wasm_bindgen(start)]
pub fn run_app() {
  yew::Renderer::<Model>::new().render();
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
