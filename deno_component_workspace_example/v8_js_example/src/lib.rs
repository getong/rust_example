use std::sync::Mutex;

use anyhow::Context as _;
use thiserror::Error;

static INITIALIZED: Mutex<bool> = Mutex::new(false);

#[derive(Error, Debug)]
pub enum Error {
  #[error("{0}")]
  V8ExceptionThrown(String),
  #[error("unknown error")]
  Unreacheable,
  #[error(transparent)]
  Other(#[from] anyhow::Error),
}

type Result<T> = std::result::Result<T, Error>;

fn ensure_v8_initialized() {
  if !*INITIALIZED.lock().unwrap() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    *INITIALIZED.lock().unwrap() = true;
  }
}

pub fn create_user_token(
  api_key: impl AsRef<str>,
  api_secret: impl AsRef<str>,
  user_id: impl AsRef<str>,
) -> Result<String> {
  const NONE_ERR_MSG: &str = "None returned during v8 processing";

  ensure_v8_initialized();

  let mut isolate = v8::Isolate::new(v8::CreateParams::default());
  v8::scope!(let scope, &mut isolate);
  let context = v8::Context::new(&scope, Default::default());
  let scope = &mut v8::ContextScope::new(scope, context);
  let scope = v8::TryCatch::new(scope);
  let scope = std::pin::pin!(scope);
  let scope = &mut scope.init();

  // A tiny inlined implementation: this is NOT Stream Chat compatible,
  // but it keeps the example compiling without a JS build pipeline.
  // The original example expected a bundled module at js/out/stream-chat-utils.mjs.
  let code = r#"
    globalThis.createUserToken = function(apiKey, apiSecret, userId) {
      return `token(${apiKey}:${apiSecret}:${userId})`;
    };
  "#;
  let source = v8::String::new(scope, code).context(NONE_ERR_MSG)?;
  let script = v8::Script::compile(scope, source, None).context(NONE_ERR_MSG)?;
  script.run(scope).context(NONE_ERR_MSG)?;

  let global = context.global(scope);
  let key = v8::String::new(scope, "createUserToken")
    .context(NONE_ERR_MSG)?
    .into();
  let func_value = global.get(scope, key).context(NONE_ERR_MSG)?;
  let func = v8::Local::<v8::Function>::try_from(func_value).context(NONE_ERR_MSG)?;

  let args = [
    v8::String::new(scope, api_key.as_ref())
      .context(NONE_ERR_MSG)?
      .into(),
    v8::String::new(scope, api_secret.as_ref())
      .context(NONE_ERR_MSG)?
      .into(),
    v8::String::new(scope, user_id.as_ref())
      .context(NONE_ERR_MSG)?
      .into(),
  ];

  if let Some(result) = func.call(scope, global.into(), &args) {
    Ok(result.to_rust_string_lossy(scope))
  } else {
    let message = {
      let key = v8::String::new(scope, "message").context(NONE_ERR_MSG)?;
      let Some(exception) = scope.exception() else {
        return Err(Error::Unreacheable);
      };
      let exception = exception.to_object(scope).context(NONE_ERR_MSG)?;
      if let Some(message) = exception.get(scope, key.into()) {
        message.to_rust_string_lossy(scope)
      } else {
        exception.to_rust_string_lossy(scope)
      }
    };
    Err(Error::V8ExceptionThrown(message))
  }
}
