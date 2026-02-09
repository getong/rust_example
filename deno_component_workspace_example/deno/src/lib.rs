// Copyright 2018-2026 the Deno authors. MIT license.

//! Library entrypoint for reusing Deno's CLI stack as an embedding API.
//!
//! This mirrors the module layout from `src/main.rs`, but exposes selected
//! modules/types for downstream crates.

pub mod args;
mod cache;
mod cdp;
mod factory;
mod file_fetcher;
mod graph_container;
mod graph_util;
mod http_util;
mod jsr;
mod lsp;
mod module_loader;
mod node;
mod npm;
mod ops;
mod registry;
mod resolver;
mod standalone;
mod task_runner;
pub mod tools;
mod tsc;
mod type_checker;
pub mod util;
mod worker;

// Compatibility re-exports for modules that refer to these via `crate::...`.
pub use factory::CliFactory;

pub fn unstable_exit_cb(feature: &str, api_name: &str) {
  eprintln!("Feature '{feature}' for '{api_name}' was not specified, exiting.");
  std::process::exit(70);
}

pub mod sys {
  #[allow(clippy::disallowed_types)] // ok, definition
  pub type CliSys = sys_traits::impls::RealSys;
}

// These are intentionally root imports in order to make them available as
// `crate::colors`, `crate::version`, and `crate::display`.
pub use deno_lib::version;
pub use deno_terminal::colors;
pub use util::display;

