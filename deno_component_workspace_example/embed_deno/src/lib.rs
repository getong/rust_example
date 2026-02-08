// Copyright 2018-2026 the Deno authors. MIT license.

//! A mostly-vendored copy of Deno's `cli/` crate, with a small embedding
//! extension to support `globalThis.embedDeno.setResult(...)`.

pub mod args;
mod cache;
mod cdp;
mod factory;
mod file_fetcher;
mod graph_container;
mod graph_util;
mod http_util;
mod jsr;
// NOTE: `src/lsp` was intentionally removed from this fork.
// Keep it commented out to avoid accidentally pulling LSP code into builds.
// mod lsp;
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

pub mod embed;

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
// `crate::colors`, `crate::version`, and `crate::display` (many of the
// vendored Deno CLI modules refer to them via `crate::...`).
pub use deno_lib::version;
pub use deno_terminal::colors;
pub use util::display;
