use dioxus::prelude::*;

const ECHO_CSS: Asset = asset!("/assets/styling/echo.css");

/// Echo component that demonstrates fullstack server functions.
#[component]
pub fn Echo() -> Element {
  let mut status = use_signal(|| String::new());

  rsx! {
      document::Link { rel: "stylesheet", href: ECHO_CSS }
      div {
          id: "echo",
          h4 { "ServerFn Echo" }
          input {
              placeholder: "Type here to echo...",
              oninput:  move |event| async move {
                  match echo_server(event.value()).await {
                      Ok(data) => status.set(format!("Server echoed: {data}")),
                      Err(error) => status.set(format!("Server unavailable: {error}")),
                  }
              },
          }

          if !status().is_empty() {
              p {
                  i { "{status}" }
              }
          }
      }
  }
}

/// Echo the user input on the server.
#[server]
async fn echo_server(input: String) -> Result<String, ServerFnError> {
  Ok(input)
}
