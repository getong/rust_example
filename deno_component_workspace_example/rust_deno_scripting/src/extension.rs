use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{OpState, extension, op2};
use tokio::sync::RwLock;

// To demonstrate sharing of state between our host application
// and our user scripts, we define a simple struct that holds an integer.
// As mentioned in the definition of our op below, certain types must be
// converted to be handed between Rust and JavaScript, meaning a direct
// sharing of memory is not always possible and some duplication might
// be required.
pub struct HostState {
  pub n: i32,
}

// Ops in Deno can be asynchronous or synchronous.
// By default, async ops are directly polled without awaiting them, which matches
// the behavior of async JavaScript functions but is different from normal Rust.
// Certain data types, such as the OpState or integers, can be directly passed between
// JavaScript and Rust.
// Especially strings can introduce some overhead, as JavaScript's UTF-16 strings must
// first be converted to Rust's expected string encoding UTF-8.
// More information about ops performance and type conversions can be found at:
//   https://docs.rs/deno_core/latest/deno_core/attr.op2.html
#[op2]
async fn op_scripting_demo(state: Rc<RefCell<OpState>>, n: i32) -> i32 {
  let lock = state.borrow().borrow::<Arc<RwLock<HostState>>>().clone();
  let mut host_state = lock.write().await;
  host_state.n += n;
  host_state.n
}

// This macro conveniently defines and implements a struct for initializing our extension.
// In other examples such as https://github.com/denoland/roll-your-own-javascript-runtime/
// you may find that the extension definition is split between a "build.rs" build script
// and runtime code.
// This is done to create a snapshot that takes care of parsing and preparing the
// extension's JavaScript code at build time to improve the program's start time.
// For this demo, we forgo this optimization.
extension!(
  my_extension,
  ops = [op_scripting_demo],
  esm_entry_point = "ext:my_extension/builtins/bootstrap.js",
  esm = ["builtins/bootstrap.js"],
  docs = "A small sample extension"
);
