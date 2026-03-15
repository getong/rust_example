#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus_router::{Routable, Router};

use crate::page::Register;

#[derive(Clone, Debug, PartialEq, Routable)]
enum Route {
  #[route("/")]
  Register {},
}

pub fn App() -> Element {
  rsx! {
      Router::<Route> {}
  }
}
