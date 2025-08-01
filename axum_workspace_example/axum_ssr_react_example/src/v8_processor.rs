use std::fs;

use ssr_rs::v8;

pub trait HttpRequest {
  fn path(&self) -> String;
  fn referrer(&self) -> String;
  fn host(&self) -> String;
  fn user_agent(&self) -> String;
}

pub struct StringHttpRequest {
  path: String,
  referrer: String,
  host: String,
  user_agent: String,
}

impl StringHttpRequest {
  pub fn new(path: &str, referrer: &str, host: &str, user_agent: &str) -> Self {
    Self {
      path: path.to_string(),
      referrer: referrer.to_string(),
      host: host.to_string(),
      user_agent: user_agent.to_string(),
    }
  }
}

impl HttpRequest for StringHttpRequest {
  fn path(&self) -> String {
    self.path.clone()
  }

  fn referrer(&self) -> String {
    self.referrer.clone()
  }

  fn host(&self) -> String {
    self.host.clone()
  }

  fn user_agent(&self) -> String {
    self.user_agent.clone()
  }
}

pub struct V8TypeScriptProcessor {
  _isolate: v8::OwnedIsolate,
  context: v8::Global<v8::Context>,
}

impl V8TypeScriptProcessor {
  pub fn new() -> Option<Self> {
    // Create isolate with proper parameters
    let params = v8::CreateParams::default().array_buffer_allocator(v8::new_default_allocator());

    let isolate = &mut v8::Isolate::new(params);
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    // Load and execute the compiled TypeScript files
    if let (Ok(v8_processing_js), Ok(data_generators_js)) = (
      fs::read_to_string("client/dist/v8/v8-processing.js"),
      fs::read_to_string("client/dist/v8/data-generators.js"),
    ) {
      // Execute v8-processing.js
      if let Some(code) = v8::String::new(scope, &v8_processing_js) {
        if let Some(script) = v8::Script::compile(scope, code, None) {
          script.run(scope);
        }
      }

      // Execute data-generators.js
      if let Some(code) = v8::String::new(scope, &data_generators_js) {
        if let Some(script) = v8::Script::compile(scope, code, None) {
          script.run(scope);
        }
      }
    }

    // Note: We can't move the isolate from a mutable reference
    // This is a limitation with the current V8 API and isolate management
    // For now, we'll return None to indicate we need a different approach
    None
  }

  pub fn process_http_request(&mut self, request: &dyn HttpRequest) -> Option<String> {
    let isolate = &mut self._isolate;
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Local::new(scope, &self.context);
    let scope = &mut v8::ContextScope::new(scope, context);

    // Get the processHttpRequest function
    let function_name = v8::String::new(scope, "processHttpRequest")?;
    let function_val = context.global(scope).get(scope, function_name.into())?;
    let function = v8::Local::<v8::Function>::try_from(function_val).ok()?;

    // Create request object
    let request_obj = v8::Object::new(scope);

    let path_key = v8::String::new(scope, "path")?;
    let path_val = v8::String::new(scope, &request.path())?;
    request_obj.set(scope, path_key.into(), path_val.into());

    let referrer_key = v8::String::new(scope, "referrer")?;
    let referrer_val = v8::String::new(scope, &request.referrer())?;
    request_obj.set(scope, referrer_key.into(), referrer_val.into());

    let host_key = v8::String::new(scope, "host")?;
    let host_val = v8::String::new(scope, &request.host())?;
    request_obj.set(scope, host_key.into(), host_val.into());

    let user_agent_key = v8::String::new(scope, "user_agent")?;
    let user_agent_val = v8::String::new(scope, &request.user_agent())?;
    request_obj.set(scope, user_agent_key.into(), user_agent_val.into());

    // Call the function
    let undefined = v8::undefined(scope);
    let args = &[request_obj.into()];
    let result = function.call(scope, undefined.into(), args)?;

    // Convert result to JSON string
    let json_stringify_name = v8::String::new(scope, "JSON")?;
    let json_obj = context
      .global(scope)
      .get(scope, json_stringify_name.into())?;
    let json_obj = v8::Local::<v8::Object>::try_from(json_obj).ok()?;

    let stringify_name = v8::String::new(scope, "stringify")?;
    let stringify_fn = json_obj.get(scope, stringify_name.into())?;
    let stringify_fn = v8::Local::<v8::Function>::try_from(stringify_fn).ok()?;

    let json_result = stringify_fn.call(scope, json_obj.into(), &[result])?;
    let json_string = json_result.to_string(scope)?;

    Some(json_string.to_rust_string_lossy(scope))
  }

  pub fn process_data_request(
    &mut self,
    request_type: &str,
    params: Option<&str>,
  ) -> Option<String> {
    let isolate = &mut self._isolate;
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Local::new(scope, &self.context);
    let scope = &mut v8::ContextScope::new(scope, context);

    // Get the processDataRequest function
    let function_name = v8::String::new(scope, "processDataRequest")?;
    let function_val = context.global(scope).get(scope, function_name.into())?;
    let function = v8::Local::<v8::Function>::try_from(function_val).ok()?;

    // Create arguments
    let request_type_val = v8::String::new(scope, request_type)?;
    let mut args = vec![request_type_val.into()];

    if let Some(params_str) = params {
      if let Some(params_val) = v8::String::new(scope, params_str) {
        // Parse params as JSON
        let json_name = v8::String::new(scope, "JSON")?;
        let json_obj = context.global(scope).get(scope, json_name.into())?;
        let json_obj = v8::Local::<v8::Object>::try_from(json_obj).ok()?;

        let parse_name = v8::String::new(scope, "parse")?;
        let parse_fn = json_obj.get(scope, parse_name.into())?;
        let parse_fn = v8::Local::<v8::Function>::try_from(parse_fn).ok()?;

        if let Some(parsed_params) = parse_fn.call(scope, json_obj.into(), &[params_val.into()]) {
          args.push(parsed_params);
        }
      }
    }

    // Call the function
    let undefined = v8::undefined(scope);
    let result = function.call(scope, undefined.into(), &args)?;

    // Convert result to JSON string
    let json_stringify_name = v8::String::new(scope, "JSON")?;
    let json_obj = context
      .global(scope)
      .get(scope, json_stringify_name.into())?;
    let json_obj = v8::Local::<v8::Object>::try_from(json_obj).ok()?;

    let stringify_name = v8::String::new(scope, "stringify")?;
    let stringify_fn = json_obj.get(scope, stringify_name.into())?;
    let stringify_fn = v8::Local::<v8::Function>::try_from(stringify_fn).ok()?;

    let json_result = stringify_fn.call(scope, json_obj.into(), &[result])?;
    let json_string = json_result.to_string(scope)?;

    Some(json_string.to_rust_string_lossy(scope))
  }
}

// Convenience functions for creating processors and processing requests
pub fn create_v8_processor() -> Option<V8TypeScriptProcessor> {
  V8TypeScriptProcessor::new()
}

pub fn process_sample_requests() -> Vec<String> {
  let mut processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor".to_string()],
  };

  let requests = vec![
    StringHttpRequest::new("/", "", "localhost:8080", "Mozilla/5.0 (Chrome/91.0)"),
    StringHttpRequest::new("/api/users", "", "api.example.com", "curl/7.64.1"),
    StringHttpRequest::new(
      "/admin/panel",
      "/dashboard",
      "admin.site.com",
      "Mozilla/5.0 (Firefox/89.0)",
    ),
    StringHttpRequest::new(
      "/images/logo.png",
      "/",
      "cdn.example.com",
      "Mozilla/5.0 (Safari/14.0)",
    ),
    StringHttpRequest::new("/old-page", "", "example.com", "googlebot"),
  ];

  requests
    .iter()
    .filter_map(|req| processor.process_http_request(req))
    .collect()
}

pub fn process_sample_data_requests() -> Vec<String> {
  let mut processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor".to_string()],
  };

  vec![
    processor
      .process_data_request("user", Some(r#"{"id": 42}"#))
      .unwrap_or_default(),
    processor
      .process_data_request("users", Some(r#"{"count": 3}"#))
      .unwrap_or_default(),
    processor
      .process_data_request("analytics", None)
      .unwrap_or_default(),
  ]
}
