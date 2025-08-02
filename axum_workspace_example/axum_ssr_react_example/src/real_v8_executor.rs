use std::fs;

use ssr_rs::v8;

use crate::config::STREAM_CONFIG;

pub struct RealV8Executor;

impl RealV8Executor {
  pub fn initialize() -> Result<(), String> {
    // V8 initialization is handled by ssr_rs::Ssr::create_platform()
    // which is already called in main(). No additional initialization needed.
    Ok(())
  }

  // Load the Stream Chat JavaScript bundle from file or use inline version
  fn load_stream_chat_bundle() -> Result<String, String> {
    let bundle_path = "client/dist/stream-chat-server.js";

    // Try to load from file first
    if let Ok(content) = fs::read_to_string(bundle_path) {
      println!("✅ Loaded Stream Chat bundle from: {}", bundle_path);
      println!("📦 Bundle size: {} bytes", content.len());

      // Check if the bundle has the functions we need
      let has_sync_func = content.contains("processStreamChatRequestSync");
      let has_render_func = content.contains("renderStreamChatHTML");
      let has_css = content.contains("const css");

      println!("🔍 Bundle validation:");
      println!(
        "  - processStreamChatRequestSync: {}",
        if has_sync_func { "✅" } else { "❌" }
      );
      println!(
        "  - renderStreamChatHTML: {}",
        if has_render_func { "✅" } else { "❌" }
      );
      println!("  - CSS styles: {}", if has_css { "✅" } else { "❌" });

      if has_sync_func && has_render_func {
        return Ok(content);
      } else {
        println!("⚠️  Bundle missing required functions, using fallback");
      }
    } else {
      println!("⚠️  Could not load bundle from: {}", bundle_path);
    }

    // Fallback to minimal working version with debug info
    println!("🔧 Using fallback Stream Chat implementation");
    Ok(r#"
    console.log("[DEBUG] Loading fallback Stream Chat implementation");
    
    function processStreamChatRequestSync(action, params) {
      console.log("[DEBUG] processStreamChatRequestSync called:", action, params);
      
      const result = {
        success: true,
        user: {
          id: params.userId || 'anonymous',
          name: 'Test User',
          role: 'user'
        },
        token: 'test_token_12345',
        api_key: params.apiKey,
        expires_at: new Date(Date.now() + 86400000).toISOString(),
        issued_at: new Date().toISOString(),
        processing_time_ms: 50,
        sdk_version: '9.14.0',
        implementation: 'Fallback V8 Implementation',
        debug_info: {
          action: action,
          params: params,
          timestamp: new Date().toISOString()
        }
      };
      
      console.log("[DEBUG] processStreamChatRequestSync result:", result);
      return result;
    }
    
    function renderStreamChatHTML(action, data) {
      console.log("[DEBUG] renderStreamChatHTML called:", action, data);
      
      const html = '<div class="stream-chat-result"><h2>✅ ' + action + ' Success (Fallback)</h2>' +
                   '<p><strong>User:</strong> ' + (data.user ? data.user.id : 'Unknown') + '</p>' +
                   '<p><strong>Token:</strong> ' + (data.token ? data.token.substring(0, 20) + '...' : 'None') + '</p>' +
                   '<p><strong>Implementation:</strong> ' + (data.implementation || 'Unknown') + '</p>' +
                   '<p><strong>Processing Time:</strong> ' + (data.processing_time_ms || 0) + 'ms</p></div>';
      
      console.log("[DEBUG] Generated HTML length:", html.length);
      return html;
    }
    
    const css = '<style>.stream-chat-result { background: #e8f5e8; border: 1px solid #4caf50; border-radius: 8px; padding: 20px; margin: 20px; font-family: Arial, sans-serif; } .stream-chat-result h2 { color: #2e7d32; margin-top: 0; }</style>';
    
    globalThis.processStreamChatRequestSync = processStreamChatRequestSync;
    globalThis.renderStreamChatHTML = renderStreamChatHTML;
    globalThis.css = css;
    
    console.log("[DEBUG] Fallback Stream Chat functions registered on globalThis");
    "#.to_string())
  }

  // Execute Stream Chat JavaScript and render to HTML
  pub fn execute_stream_chat_js(action: &str, user_id: Option<&str>) -> Result<String, String> {
    // Create a new isolate for each request to avoid thread safety issues
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    // Load the actual Stream Chat bundle
    let stream_chat_bundle = Self::load_stream_chat_bundle()?;

    // Combine the bundle with execution code
    let js_code = format!(
      r#"
            // Load the Stream Chat SDK bundle
            {}

            // Execute the Stream Chat function and render HTML
            (async function() {{
                try {{
                    const htmlResult = await processStreamChatAndRenderHTML(
                        '{}', 
                        '{}', 
                        '{}', 
                        '{}'
                    );
                    return css + htmlResult;
                }} catch (error) {{
                    return `
                        <div class="stream-chat-error">
                            <h2>❌ Stream Chat SDK Error</h2>
                            <p class="error-message">${{error.message || error}}</p>
                            <div class="metadata">
                                <p><strong>Action:</strong> {}</p>
                                <p><strong>Error Type:</strong> ${{error.name || 'Unknown'}}</p>
                                <p><strong>Executed via:</strong> Real Stream Chat SDK in V8</p>
                            </div>
                        </div>
                        <style>
                            .stream-chat-error {{ 
                                background: #fee; 
                                border: 1px solid #f88; 
                                border-radius: 8px; 
                                padding: 20px; 
                                margin: 20px; 
                                font-family: Arial, sans-serif; 
                            }}
                            .error-message {{ color: #c00; font-weight: bold; }}
                            .metadata {{ background: #f9f9f9; padding: 10px; border-radius: 4px; margin-top: 15px; }}
                        </style>
                    `;
                }}
            }})()
            "#,
      stream_chat_bundle,
      action,
      user_id.unwrap_or("anonymous"),
      STREAM_CONFIG.api_key,
      STREAM_CONFIG.api_secret,
      action
    );

    // Execute JavaScript
    let code = v8::String::new(scope, &js_code).ok_or("Failed to create JS string")?;
    let script = v8::Script::compile(scope, code, None).ok_or("Failed to compile JS")?;
    let result = script.run(scope).ok_or("Failed to execute JS")?;

    // Handle Promise result (since we're using async function)
    if result.is_promise() {
      let promise = v8::Local::<v8::Promise>::try_from(result)
        .map_err(|_| "Failed to convert result to Promise")?;

      // For now, return a placeholder since we can't await in V8 without an event loop
      // In a production environment, you'd need to set up a proper event loop
      let promise_result = promise.result(scope);
      if promise_result.is_undefined() {
        return Err(
          "Promise is still pending - V8 async execution not fully supported".to_string(),
        );
      }

      let result_str = promise_result
        .to_string(scope)
        .ok_or("Failed to convert Promise result")?;
      let rust_string = result_str.to_rust_string_lossy(scope);
      Ok(rust_string)
    } else {
      // Convert result to string for synchronous execution
      let result_str = result.to_string(scope).ok_or("Failed to convert result")?;
      let rust_string = result_str.to_rust_string_lossy(scope);
      Ok(rust_string)
    }
  }

  // Synchronous version that handles async functions by converting them to sync
  pub fn execute_stream_chat_js_sync(
    action: &str,
    user_id: Option<&str>,
  ) -> Result<String, String> {
    println!(
      "🚀 Starting V8 execution for action: {}, user: {:?}",
      action, user_id
    );

    // Create a new isolate for each request to avoid thread safety issues
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    // Load the actual Stream Chat bundle
    let stream_chat_bundle = Self::load_stream_chat_bundle()?;

    // Execute the bundled Stream Chat code
    let js_code = format!(
      r#"
            console.log("[V8] Loading Stream Chat bundle...");
            
            // Load the Bun-bundled Stream Chat SDK
            {}
            
            console.log("[V8] Bundle loaded, checking globals...");
            console.log("[V8] processStreamChatRequestSync available:", typeof processStreamChatRequestSync);
            console.log("[V8] renderStreamChatHTML available:", typeof renderStreamChatHTML);
            console.log("[V8] css available:", typeof css);

            // Synchronous execution wrapper
            try {{
                console.log("[V8] Starting execution for action: {}", "with user: {}");
                
                // Wait a bit for the bundle to initialize
                if (typeof processStreamChatRequestSync === 'undefined') {{
                    throw new Error('Stream Chat bundle not properly loaded - processStreamChatRequestSync not found');
                }}
                
                // Execute Stream Chat processing synchronously
                console.log("[V8] Calling processStreamChatRequestSync...");
                const data = processStreamChatRequestSync('{}', {{
                    userId: '{}',
                    apiKey: '{}',
                    apiSecret: '{}'
                }});
                
                console.log("[V8] Processing completed, success:", data.success);
                
                // Render the HTML
                console.log("[V8] Rendering HTML...");
                const htmlResult = renderStreamChatHTML('{}', data);
                
                console.log("[V8] HTML rendered, length:", htmlResult.length);
                
                // Return CSS + HTML
                const finalResult = (css || '') + htmlResult;
                console.log("[V8] Final result prepared, total length:", finalResult.length);
                
                finalResult;
                
            }} catch (error) {{
                console.log("[V8] Error occurred:", error.message);
                const errorHtml = `<div class="stream-chat-error">
                    <h2>❌ Stream Chat SDK Error (V8 Execution)</h2>
                    <p class="error-message">${{error.message || error}}</p>
                    <div class="metadata">
                        <p><strong>Action:</strong> {}</p>
                        <p><strong>User:</strong> {}</p>
                        <p><strong>Error Type:</strong> ${{error.name || 'Unknown'}}</p>
                        <p><strong>Bundle Path:</strong> client/dist/stream-chat-server.js</p>
                        <p><strong>Executed via:</strong> Real V8 Engine with Debug Info</p>
                        <p><strong>Timestamp:</strong> ${{new Date().toISOString()}}</p>
                    </div>
                </div>
                <style>
                    .stream-chat-error {{ 
                        background: #fee; 
                        border: 1px solid #f88; 
                        border-radius: 8px; 
                        padding: 20px; 
                        margin: 20px; 
                        font-family: Arial, sans-serif; 
                    }}
                    .error-message {{ color: #c00; font-weight: bold; }}
                    .metadata {{ background: #f9f9f9; padding: 10px; border-radius: 4px; margin-top: 15px; }}
                </style>`;
                
                console.log("[V8] Error HTML prepared");
                errorHtml;
            }}
            "#,
      stream_chat_bundle,
      action,
      user_id.unwrap_or("anonymous"),
      action,
      user_id.unwrap_or("anonymous"),
      STREAM_CONFIG.api_key,
      STREAM_CONFIG.api_secret,
      action,
      action,
      user_id.unwrap_or("anonymous")
    );

    println!(
      "📝 Executing JavaScript code (length: {} chars)",
      js_code.len()
    );

    // Execute JavaScript
    let code = v8::String::new(scope, &js_code).ok_or("Failed to create JS string")?;
    let script = v8::Script::compile(scope, code, None).ok_or("Failed to compile JS")?;
    let result = script.run(scope).ok_or("Failed to execute JS")?;

    // Convert result to string
    let result_str = result.to_string(scope).ok_or("Failed to convert result")?;
    let rust_string = result_str.to_rust_string_lossy(scope);

    println!(
      "✅ V8 execution completed. Result length: {} chars",
      rust_string.len()
    );
    println!(
      "📊 First 100 chars of result: {}",
      if rust_string.len() > 100 {
        &rust_string[.. 100]
      } else {
        &rust_string
      }
    );

    Ok(rust_string)
  }
}
