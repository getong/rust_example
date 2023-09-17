use dioxus::prelude::*;
use dioxus_desktop::Config;
use anyhow::Result;

pub fn main() -> Result<()> {
    // init_logging();

    // Right now we're going through dioxus-desktop but we'd like to go through dioxus-mobile
    // That will seed the index.html with some fixes that prevent the page from scrolling/zooming etc
    dioxus_desktop::launch_cfg(
        app,
        // Note that we have to disable the viewport goofiness of the browser.
        // Dioxus_mobile should do this for us
        Config::default().with_custom_index(r#"<!DOCTYPE html>
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
    );

    Ok(())
}

fn app(cx: Scope) -> Element {
    let items = cx.use_hook(|| vec![1, 2, 3]);

    log::debug!("Hello from the app");

    render! {
        div {
            h1 { "Hello, Mobile"}
            div { margin_left: "auto", margin_right: "auto", width: "200px", padding: "10px", border: "1px solid black",
                  button {
                      onclick: move|_| {
                          println!("Clicked!");
                          items.push(items.len());
                          cx.needs_update_any(ScopeId(0));
                          println!("Requested update");
                      },
                      "Add item"
                  }
                  for item in items.iter() {
                      div { "- {item}" }
                  }
            }
        }
    }
}

// copy from https://dioxuslabs.com/learn/0.4/getting_started/mobile
// cargo apple open
// cargo android open