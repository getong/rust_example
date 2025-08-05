use std::{cell::UnsafeCell, sync::Mutex};

use anyhow::Context as _;
use thiserror::Error;
use v8::{Context, Global, OwnedIsolate};

fn create_origin<'s>(
  scope: &mut v8::HandleScope<'s>,
  filename: impl AsRef<str>,
  is_module: bool,
) -> v8::ScriptOrigin<'s> {
  let name: v8::Local<'s, v8::Value> = v8::String::new(scope, filename.as_ref()).unwrap().into();
  v8::ScriptOrigin::new(
    scope,
    name,
    0,
    0,
    false,
    0,
    Some(name),
    false,
    false,
    is_module,
    None,
  )
}

fn module_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  name: v8::Local<'s, v8::String>,
  _arr: v8::Local<'s, v8::FixedArray>,
  _module: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  let scope = &mut unsafe { v8::CallbackScope::new(context) };
  let name_str = name.to_rust_string_lossy(scope);

  // Handle node: imports by providing empty modules
  if name_str.starts_with("node:") {
    let source = match name_str.as_str() {
      "node:module" => "export function createRequire() { return () => {} }",
      _ => "export default {}",
    };

    let source = v8::String::new(scope, source).unwrap();
    let origin = create_origin(scope, &name_str, true);
    let mut source = v8::script_compiler::Source::new(source, Some(&origin));

    if let Some(module) = v8::script_compiler::compile_module(scope, &mut source) {
      let _ = module.instantiate_module(scope, module_callback);
      let _ = module.evaluate(scope);
      return Some(module);
    }
  }

  None
}

static INITIALIZED: Mutex<bool> = Mutex::new(false);

/// Exceptions related to this crate
#[derive(Error, Debug)]
pub enum Error {
  /// Error with exception thrown from V8
  #[error("{0}")]
  V8ExceptionThrown(String),
  /// Unknown error
  #[error("unknown error")]
  Unreacheable,
  /// Other error
  #[error(transparent)]
  Other(#[from] anyhow::Error),
}

/// local shortcode of Result
type Result<T> = std::result::Result<T, Error>;

const CREATE_USER_TOKEN_ID: &str = "createUserToken"; // function name for createUserToken

/// Variables to keep from V8 initialization
type InitializationResults = (UnsafeCell<OwnedIsolate>, Global<Context>);

/// Initialization: JIT compilation and function object registration
fn initialize() -> InitializationResults {
  if !*INITIALIZED.lock().unwrap() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    *INITIALIZED.lock().unwrap() = true;
  }

  let mut isolate = v8::Isolate::new(Default::default());
  let global_context;
  {
    let handle_scope = &mut v8::HandleScope::new(&mut isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    global_context = Global::new(handle_scope, context);
    {
      // JIT compilation and function object registration

      // Generate scope
      let context = v8::Local::new(handle_scope, context);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

      // Load js file
      let code = include_str!("../js/out/stream-chat-utils.mjs");
      let source = v8::String::new(scope, code).unwrap();
      let origin = create_origin(scope, "index.js", true);
      let mut source = v8::script_compiler::Source::new(source, Some(&origin));
      let module = v8::script_compiler::compile_module(scope, &mut source).unwrap();

      // Instantiate module
      module.instantiate_module(scope, module_callback).unwrap();
      module.evaluate(scope).unwrap();

      // Get the module namespace
      let namespace = module.get_module_namespace().to_object(scope).unwrap();

      // Register createUserToken export
      let create_user_token_key = v8::String::new(scope, "createUserToken").unwrap();
      if let Some(create_user_token_export) = namespace.get(scope, create_user_token_key.into()) {
        let key = v8::String::new(scope, CREATE_USER_TOKEN_ID).unwrap().into();
        context
          .global(scope)
          .set(scope, key, create_user_token_export);
      }
    }
  }
  (UnsafeCell::new(isolate), global_context)
}

/// Create a user token for Stream Chat
pub fn create_user_token(
  api_key: impl AsRef<str>,
  api_secret: impl AsRef<str>,
  user_id: impl AsRef<str>,
) -> Result<String> {
  /// Error message when casting None to Result
  const NONE_ERR_MSG: &str = "None returned during v8 processing";

  thread_local! {
      pub static ISOLATE_CONTEXT: InitializationResults = initialize();
  };
  ISOLATE_CONTEXT.with(|(isolate, context): &InitializationResults| {
    let isolate: &mut OwnedIsolate = unsafe { isolate.get().as_mut().unwrap_unchecked() };

    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Local::new(handle_scope, context.clone());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    let scope = &mut v8::TryCatch::new(scope);

    let global = context.global(scope);
    let key = v8::String::new(scope, CREATE_USER_TOKEN_ID)
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
  })
}
