use gpui::{App, Context, Window, WindowOptions, div, prelude::*};
use gpui_router::prelude::*;

struct HelloWorld {}

impl Render for HelloWorld {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .child(
        div()
          .child(NavLink::new().to("/").child(div().child("Home")))
          .child(NavLink::new().to("/about").child(div().child("About")))
          .child(
            NavLink::new()
              .to("/dashboard")
              .child(div().child("Dashboard")),
          )
          .child(
            NavLink::new()
              .to("/nothing-here")
              .child(div().child("Not Match")),
          ),
      )
      .child(
        Routes::new()
          .basename("/")
          .child(Route::new().index().element(|_, _| home()))
          .child(Route::new().path("about").element(|_, _| about()))
          .child(Route::new().path("dashboard").element(|_, _| dashboard()))
          .child(
            Route::new()
              .path("{*not_match}")
              .element(|_, _| not_match()),
          ),
      )
  }
}

fn home() -> impl IntoElement {
  div().child("Home")
}

fn about() -> impl IntoElement {
  div().child("About")
}

fn dashboard() -> impl IntoElement {
  div().child("Dashboard")
}

fn not_match() -> impl IntoElement {
  div().child(div().child("Nothing to see here!")).child(
    NavLink::new()
      .to("/")
      .child(div().child("Go to the home page")),
  )
}

fn main() {
  gpui_platform::application().run(|cx: &mut App| {
    init(cx);
    cx.activate(true);
    cx.open_window(WindowOptions::default(), |_, cx| {
      cx.new(|_cx| HelloWorld {})
    })
    .unwrap();
  });
}
