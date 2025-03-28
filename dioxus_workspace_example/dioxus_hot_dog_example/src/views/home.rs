use dioxus::prelude::*;

use crate::components::{Echo, Hero};

#[component]
pub fn Home() -> Element {
  rsx! {
      Hero {}
      Echo {}
  }
}
