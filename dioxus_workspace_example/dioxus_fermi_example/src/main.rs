mod app;
mod page;

#[cfg(target_arch = "wasm32")]
fn main() {
  dioxus::launch(app::App);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  dioxus_desktop::launch::launch(
    app::App,
    Vec::<Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>>::new(),
    Vec::<Box<dyn std::any::Any>>::new(),
  );
}
