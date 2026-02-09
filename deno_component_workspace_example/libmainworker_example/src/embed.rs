// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::{Arc, Mutex};

use deno_core::{Extension, OpState, op2};

#[derive(Clone, Default)]
pub struct EmbedResult {
  pub result: Option<String>,
  pub exit_data: Option<String>,
}

#[op2(fast)]
fn libmainworker_set_result(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.result = Some(value);
  }
}

#[op2(fast)]
fn libmainworker_set_exit_data(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.exit_data = Some(value);
  }
}

deno_core::extension!(
  libmainworker_ext,
  ops = [libmainworker_set_result, libmainworker_set_exit_data],
  esm_entry_point = "ext:libmainworker_ext/embed/result.js",
  esm = [dir "src", "embed/result.js"],
  options = {
    result_holder: Arc<Mutex<EmbedResult>>,
  },
  state = |state, options| {
    state.put(options.result_holder);
  }
);

pub fn extension(result_holder: Arc<Mutex<EmbedResult>>) -> Extension {
  libmainworker_ext::init(result_holder)
}
