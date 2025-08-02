use ssr_rs::Ssr;

use crate::config::STREAM_CONFIG;

// Execute Stream Chat TypeScript functions using ssr_rs
pub struct StreamChatExecutor;

impl StreamChatExecutor {
  pub fn execute_function(
    function_name: &str,
    params: serde_json::Value,
  ) -> Result<String, String> {
    // Try to load the server-side implementation first
    let typescript_code = match std::fs::read_to_string("client/dist/v8/stream-chat-server.js") {
      Ok(code) => code,
      Err(_) => {
        // Fallback to the demo file
        std::fs::read_to_string("client/dist/v8/stream-chat-demo.js")
          .map_err(|e| format!("Failed to load TypeScript code: {}", e))?
      }
    };

    // Create the JavaScript to execute - wrap in a function that returns the result
    let js_code = format!(
      r#"
            // Load the Stream Chat implementation
            {}
            
            // Create a function that SSR can execute
            function App() {{
              const params = {};
              const result = processStreamChatRequest('{}', params);
              return JSON.stringify(result);
            }}
            
            // Export for SSR
            if (typeof module !== 'undefined') {{
              module.exports = {{ default: App }};
            }}
            "#,
      typescript_code,
      serde_json::to_string(&params).unwrap(),
      function_name
    );

    // Create SSR instance and execute
    let mut ssr = Ssr::from(js_code, "stream-chat")
      .map_err(|e| format!("Failed to create SSR instance: {:?}", e))?;

    let result = ssr
      .render_to_string(None)
      .map_err(|e| format!("Failed to execute: {:?}", e))?;

    Ok(result)
  }

  pub fn authenticate_user(user_id: &str) -> Result<String, String> {
    let params = serde_json::json!({
        "user_id": user_id,
        "api_key": &STREAM_CONFIG.api_key,
        "api_secret": &STREAM_CONFIG.api_secret
    });

    Self::execute_function("authenticate", params)
  }

  pub fn get_user_context(user_id: &str) -> Result<String, String> {
    let params = serde_json::json!({
        "user_id": user_id,
        "api_key": &STREAM_CONFIG.api_key,
        "api_secret": &STREAM_CONFIG.api_secret
    });

    Self::execute_function("user-context", params)
  }

  pub fn get_analytics() -> Result<String, String> {
    let params = serde_json::json!({
        "api_key": &STREAM_CONFIG.api_key,
        "api_secret": &STREAM_CONFIG.api_secret
    });

    Self::execute_function("analytics", params)
  }

  pub fn get_setup() -> Result<String, String> {
    let params = serde_json::json!({
        "api_key": &STREAM_CONFIG.api_key,
        "api_secret": &STREAM_CONFIG.api_secret
    });

    Self::execute_function("setup", params)
  }
}
