#![allow(non_snake_case)]

use dioxus::prelude::*;

pub fn Register() -> Element {
  let mut count = use_signal(|| 0);
  let mut name = use_signal(|| String::from("Dioxus"));

  rsx! {
      div {
          display: "flex",
          flex_direction: "column",
          width: "100%",
          height: "100%",
          padding: "24px",
          gap: "12px",
          background_color: "rgb(248, 250, 252)",

          h1 {
              font_size: "28px",
              color: "rgb(15, 23, 42)",
              "Fermi Example"
          }

          p {
              color: "rgb(71, 85, 105)",
              "This example now renders a visible page instead of an empty string."
          }

          p {
              color: "rgb(30, 41, 59)",
              "Hello, {name}."
          }

          p {
              color: "rgb(30, 41, 59)",
              "Counter: {count}"
          }

          div {
              display: "flex",
              gap: "8px",

              button {
                  onclick: move |_| count += 1,
                  padding: "8px 12px",
                  background_color: "rgb(37, 99, 235)",
                  color: "white",
                  border: "none",
                  border_radius: "8px",
                  "Increment"
              }

              button {
                  onclick: move |_| {
                      name.set(if name() == "Dioxus" {
                          "Desktop".to_string()
                      } else {
                          "Dioxus".to_string()
                      });
                  },
                  padding: "8px 12px",
                  background_color: "rgb(15, 23, 42)",
                  color: "white",
                  border: "none",
                  border_radius: "8px",
                  "Toggle name"
              }
          }
      }
  }
}
