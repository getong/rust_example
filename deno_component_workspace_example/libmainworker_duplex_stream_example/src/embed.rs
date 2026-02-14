use std::sync::{Arc, Mutex};

use deno_core::{ExtensionFileSource, OpState, op2};

const EMBED_RESULT_SPECIFIER: &str = "ext:libmainworker_embed_ext/embed_result.ts";
const EMBED_RESULT_SOURCE: &str = include_str!("embed_result.ts");

#[derive(Clone, Default)]
pub(crate) struct EmbedResult {
  pub(crate) result: Option<String>,
  pub(crate) exit_data: Option<String>,
}

#[op2(fast)]
fn libmainworker_embed_set_result(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.result = Some(value);
  }
}

#[op2(fast)]
fn libmainworker_embed_set_exit_data(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.exit_data = Some(value);
  }
}

deno_core::extension!(
  libmainworker_embed_ext,
  ops = [libmainworker_embed_set_result, libmainworker_embed_set_exit_data],
  options = {
    result_holder: Arc<Mutex<EmbedResult>>,
  },
  state = |state, options| {
    state.put(options.result_holder);
  }
);

pub(crate) fn embed_extension(result_holder: Arc<Mutex<EmbedResult>>) -> deno_core::Extension {
  let mut ext = libmainworker_embed_ext::init(result_holder);
  ext
    .esm_files
    .to_mut()
    .push(ExtensionFileSource::new_computed(
      EMBED_RESULT_SPECIFIER,
      Arc::<str>::from(EMBED_RESULT_SOURCE),
    ));
  ext.esm_entry_point = Some(EMBED_RESULT_SPECIFIER);
  ext
}
