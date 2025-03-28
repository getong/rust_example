use dioxus::prelude::*;

const ECHO_CSS: Asset = asset!("/assets/styling/echo.css");

/// Echo component that demonstrates fullstack server functions.
#[component]
pub fn Echo() -> Element {
  let mut response = use_signal(|| String::new());

  rsx! {
      document::Link { rel: "stylesheet", href: ECHO_CSS }
      div {
          id: "echo",
          h4 { "ServerFn Echo" }
          input {
              placeholder: "Type here to echo...",
              oninput:  move |event| async move {
                  let data = echo_server(event.value()).await.unwrap();
                  response.set(data);
              },
          }

          if !response().is_empty() {
              p {
                  "Server echoed: "
                  i { "{response}" }
              }
          }
      }
  }
}

/// Echo the user input on the server.
#[server(EchoServer)]
async fn echo_server(input: String) -> Result<String, ServerFnError> {
  Ok(input)
}
