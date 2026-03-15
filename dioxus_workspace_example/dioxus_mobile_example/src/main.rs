use dioxus::prelude::*;
use dioxus_desktop::Config;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
  // init_logging();

  // Right now we're going through dioxus-desktop but we'd like to go through dioxus-mobile
  // That will seed the index.html with some fixes that prevent the page from scrolling/zooming etc
  dioxus_desktop::launch::launch(
    app,
    Vec::<Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>>::new(),
    vec![Box::new(
      // Note that we have to disable the viewport goofiness of the browser.
      // Dioxus_mobile should do this for us
      Config::new().with_custom_index(r#"<!DOCTYPE html>
        <html>
          <head>
            <title>Dioxus app</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
            <!-- CUSTOM HEAD -->
          </head>
          <body>
            <div id="main"></div>
            <!-- MODULE LOADER -->
          </body>
        </html>
       "#.into()),
    )],
  );
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
  dioxus::launch(app);
}

fn app() -> Element {
  let mut items = use_signal(|| vec![1, 2, 3]);

  log::debug!("Hello from the app");

  rsx! {
      div {
          h1 { "Hello, Mobile"}
          div {
                margin_left: "auto",
                margin_right: "auto",
                width: "200px",
                padding: "10px",
                border: "1px solid black",
                button {
                    onclick: move|_| {
                        println!("Clicked!");
                        items.with_mut(|items| items.push(items.len() + 1));
                    },
                    "Add item"
                }
                for item in items().iter() {
                    div { "- {item}" }
                }
          }
      }
  }
}

// copy from https://dioxuslabs.com/learn/0.4/getting_started/mobile
// cargo apple open
// cargo android open
