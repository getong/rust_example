#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus_router::Router;
use fermi::use_init_atom_root;
use crate::page;

pub fn App() -> Element {
  rsx! {
      Router{
          Router { to: page::ACCOUNT_REGISTER, page::Register{} }
      }
  }
}
