mod pages;

use pages::{home::Home, projects::Projects, users::Users};
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Routable, Debug, Clone, PartialEq)]
pub enum Route {
  #[at("/users")]
  Users,
  #[at("/projects")]
  Projects,
  #[at("/")]
  Home,
}

fn switch(switch: Route) -> Html {
  match switch {
    Route::Users => {
      html! { <Users/> }
    }
    Route::Projects => {
      html! { <Projects/> }
    }
    Route::Home => {
      html! { <Home /> }
    }
  }
}

struct App;

impl Component for App {
  type Message = ();
  type Properties = ();

  fn create(_: &Context<Self>) -> Self {
    Self
  }

  fn update(&mut self, _: &Context<Self>, _: Self::Message) -> bool {
    false
  }

  fn view(&self, _: &Context<Self>) -> Html {
    let home_cls = "nav";

    html! {
        <>
        <div class="logo-title">
            <img src="imgs/budshome.png" />
            { "tide-async-graphql-mongodb / frontend-yew" }
        </div>
        <BrowserRouter>
        <div class={home_cls}>
            <Link<Route> to={Route::Users}>
                { "用户列表" }
            </Link<Route>>
            <span class="placeholder">{ " - " }</span>
            <Link<Route> to={Route::Projects}>
                { "项目列表" }
            </Link<Route>>
            <span class="placeholder">{ " - " }</span>
            <Link<Route> to={Route::Home}>
                { "主页" }
            </Link<Route>>
        </div>
        <main>
            <Switch<Route> render={switch} />
        </main>
        </BrowserRouter>
        </>
    }
  }
}

fn main() {
  yew::Renderer::<App>::new().render();
}
