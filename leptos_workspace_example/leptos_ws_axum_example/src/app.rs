use leptos::{prelude::*, task::spawn_local};

use crate::messages::{Message, Messages};

#[component]
pub fn MessageComp(message: Message) -> impl IntoView {
  view! {
      <div class="message">
          <p>{move || message.text()}</p>
      </div>
  }
}

#[component]
pub fn App() -> impl IntoView {
  // Provide websocket connection
  leptos_ws::provide_websocket("ws://localhost:3000/ws");
  let messages = leptos_ws::ServerSignal::new("messages".to_string(), Messages::new()).unwrap();
  let new_message = RwSignal::new("".to_string());
  view! {
      <div class="messages">
          <div class="messages_inner">
              <For
                  each=move || messages.get().get().clone().into_iter().enumerate()
                  key=move |(index, text)| (index.clone(), text.text())
                  let:data
              >
                  <MessageComp message=data.1.clone() />
              </For>
          </div>
      </div>
      <div class="new_message">
          <h3>New Message</h3>
          <div class="column">
              <div class="form-input">
                  <label for="text">Message</label>
                  <input
                      id="text"
                      type="text"
                      prop:value=new_message
                      on:input=move |e| {
                          let mut text = event_target_value(&e);
                          text.truncate(500);
                          new_message.set(text)
                      }
                      on:keypress=move |e| {
                          if e.key() == "Enter" {
                              spawn_local(async move {
                                  let _ = add_message(new_message.get_untracked()).await;
                                  new_message.set("".to_string());
                              });
                          }
                      }
                  />
              </div>
              <button on:click=move |_| spawn_local(async move {
                  let _ = add_message(new_message.get_untracked()).await;
                  new_message.set("".to_string());
              })>Send</button>
          </div>
      </div>
  }
}

#[server]
async fn add_message(message: String) -> Result<(), ServerFnError> {
  let messages = leptos_ws::ServerSignal::new("messages".to_string(), Messages::new()).unwrap();
  messages.update(move |x| {
    x.add_message(Message::new(message));
  });
  log::warn!("len: {}", messages.get().len());
  Ok(())
}
