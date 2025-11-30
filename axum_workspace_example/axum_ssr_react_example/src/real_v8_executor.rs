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
    let bundle_path = "client/dist/v8/stream-chat-demo.js";

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
    println!("file: {}, line :{}", file!(), line!());
    // Create a new isolate for each request to avoid thread safety issues
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    println!("file: {}, line :{}", file!(), line!());
    // Load the actual Stream Chat bundle
    let stream_chat_bundle = Self::load_stream_chat_bundle()?;

    // Add custom console implementation to capture console.log output
    let console_setup = r#"
    // Custom console implementation that captures output for Rust
    globalThis.console = {
      log: function(...args) {
        // Convert all arguments to strings and join them
        const message = args.map(arg => {
          if (typeof arg === 'object') {
            try {
              return JSON.stringify(arg);
            } catch (e) {
              return String(arg);
            }
          }
          return String(arg);
        }).join(' ');
        
        // Store in a global variable that Rust can access
        if (!globalThis._consoleOutput) {
          globalThis._consoleOutput = [];
        }
        globalThis._consoleOutput.push('[JS] ' + message);
      }
    };
    "#;

    // Add browser polyfills before loading Stream Chat SDK
    let browser_polyfills = r#"
    // Browser polyfills for V8 environment
    if (typeof window === 'undefined') {
      globalThis.window = globalThis;
      globalThis.self = globalThis;
      // Mock DOM element class
      class MockElement {
        constructor(tagName) {
          this.tagName = tagName;
          this.attributes = {};
          this.style = {};
          this.classList = {
            add: () => {},
            remove: () => {},
            contains: () => false
          };
          this.children = [];
          this.innerHTML = '';
          this.textContent = '';
          this.href = '';
        }
        setAttribute(name, value) {
          this.attributes[name] = value;
          if (name === 'href') {
            this.href = value;
            // Parse URL for Stream Chat SDK
            try {
              const url = new URL(value);
              this.protocol = url.protocol;
              this.host = url.host;
              this.hostname = url.hostname;
              this.port = url.port;
              this.pathname = url.pathname;
              this.search = url.search;
              this.hash = url.hash;
            } catch (e) {
              // If URL parsing fails, use defaults
              this.protocol = 'http:';
              this.host = 'localhost';
              this.hostname = 'localhost';
              this.port = '';
              this.pathname = '/';
              this.search = '';
              this.hash = '';
            }
          }
        }
        getAttribute(name) {
          return this.attributes[name];
        }
        addEventListener() {}
        removeEventListener() {}
        appendChild() {}
        removeChild() {}
        querySelector() { return null; }
        querySelectorAll() { return []; }
      }
      
      globalThis.document = {
        createElement: (tagName) => new MockElement(tagName),
        documentElement: new MockElement('html'),
        head: new MockElement('head'),
        body: new MockElement('body'),
        addEventListener: () => {},
        removeEventListener: () => {},
        querySelector: () => null,
        querySelectorAll: () => [],
        getElementById: () => null,
        getElementsByTagName: () => [],
        getElementsByClassName: () => []
      };
      globalThis.navigator = {
        userAgent: 'V8-StreamChat/1.0',
        language: 'en-US'
      };
      globalThis.location = {
        href: 'http://localhost:8080',
        origin: 'http://localhost:8080',
        protocol: 'http:',
        host: 'localhost:8080',
        hostname: 'localhost',
        port: '8080',
        pathname: '/',
        search: '',
        hash: ''
      };
      globalThis.localStorage = {
        getItem: () => null,
        setItem: () => {},
        removeItem: () => {},
        clear: () => {},
        length: 0,
        key: () => null
      };
      globalThis.sessionStorage = {
        getItem: () => null,
        setItem: () => {},
        removeItem: () => {},
        clear: () => {},
        length: 0,
        key: () => null
      };
      
      // Add XMLHttpRequest polyfill
      globalThis.XMLHttpRequest = class {
        open() {}
        send() {}
        setRequestHeader() {}
        addEventListener() {}
        removeEventListener() {}
      };
      
      // Add WebSocket polyfill
      globalThis.WebSocket = class {
        constructor(url) {
          console.log("[V8 WebSocket Mock] Connection to:", url);
        }
        send() {}
        close() {}
        addEventListener() {}
        removeEventListener() {}
      };
      
      // Add crypto polyfill
      globalThis.crypto = {
        getRandomValues: (arr) => {
          for (let i = 0; i < arr.length; i++) {
            arr[i] = Math.floor(Math.random() * 256);
          }
          return arr;
        },
        subtle: {
          digest: async () => new ArrayBuffer(32)
        }
      };
      
      // Add URL polyfill
      if (typeof URL === 'undefined') {
        globalThis.URL = class {
          constructor(url, base) {
            this.href = url;
            // Simple URL parsing
            const match = url.match(/^(https?:)\/\/([^\/]+)(\/[^?#]*)(\?[^#]*)?(#.*)?$/);
            if (match) {
              this.protocol = match[1];
              this.host = match[2];
              this.pathname = match[3] || '/';
              this.search = match[4] || '';
              this.hash = match[5] || '';
              this.hostname = this.host.split(':')[0];
              this.port = this.host.split(':')[1] || '';
            } else {
              this.protocol = 'http:';
              this.host = 'localhost';
              this.hostname = 'localhost';
              this.port = '';
              this.pathname = '/';
              this.search = '';
              this.hash = '';
            }
            this.origin = this.protocol + '//' + this.host;
          }
          toString() { return this.href; }
        };
      }
      
      // Add encode/decode URI component if missing
      if (typeof encodeURIComponent === 'undefined') {
        globalThis.encodeURIComponent = (str) => {
          return str.replace(/[!'()*]/g, (c) => {
            return '%' + c.charCodeAt(0).toString(16);
          });
        };
      }
      
      if (typeof decodeURIComponent === 'undefined') {
        globalThis.decodeURIComponent = (str) => {
          return str.replace(/%([0-9A-F]{2})/g, (match, p1) => {
            return String.fromCharCode(parseInt(p1, 16));
          });
        };
      }
      
      // Add URLSearchParams polyfill
      if (typeof URLSearchParams === 'undefined') {
        globalThis.URLSearchParams = class {
          constructor(init) {
            this.params = {};
            if (typeof init === 'string') {
              // Remove leading '?'
              init = init.replace(/^\?/, '');
              // Parse query string
              init.split('&').forEach(pair => {
                const [key, value] = pair.split('=');
                if (key) {
                  this.params[decodeURIComponent(key)] = decodeURIComponent(value || '');
                }
              });
            } else if (init && typeof init === 'object') {
              // Handle object initialization
              Object.entries(init).forEach(([key, value]) => {
                this.params[key] = String(value);
              });
            }
          }
          get(key) {
            return this.params[key] || null;
          }
          set(key, value) {
            this.params[key] = String(value);
          }
          has(key) {
            return key in this.params;
          }
          delete(key) {
            delete this.params[key];
          }
          append(key, value) {
            // For simplicity, just set (not handling multiple values)
            this.params[key] = String(value);
          }
          toString() {
            return Object.entries(this.params)
              .map(([key, value]) => encodeURIComponent(key) + '=' + encodeURIComponent(value))
              .join('&');
          }
          forEach(callback) {
            Object.entries(this.params).forEach(([key, value]) => {
              callback(value, key, this);
            });
          }
          entries() {
            return Object.entries(this.params)[Symbol.iterator]();
          }
          keys() {
            return Object.keys(this.params)[Symbol.iterator]();
          }
          values() {
            return Object.values(this.params)[Symbol.iterator]();
          }
        };
      }
      
      // Add FormData polyfill
      globalThis.FormData = class {
        constructor() {
          this.data = {};
        }
        append(key, value) {
          this.data[key] = value;
        }
        get(key) {
          return this.data[key];
        }
      };
      
      // Add Headers polyfill
      globalThis.Headers = class {
        constructor(init) {
          this.headers = {};
          if (init) {
            Object.entries(init).forEach(([key, value]) => {
              this.headers[key.toLowerCase()] = value;
            });
          }
        }
        set(key, value) {
          this.headers[key.toLowerCase()] = value;
        }
        get(key) {
          return this.headers[key.toLowerCase()];
        }
        has(key) {
          return key.toLowerCase() in this.headers;
        }
        delete(key) {
          delete this.headers[key.toLowerCase()];
        }
      };
      
      // Add Request/Response polyfills
      globalThis.Request = class {
        constructor(url, init) {
          this.url = url;
          this.method = (init && init.method) || 'GET';
          this.headers = new Headers(init && init.headers);
          this.body = init && init.body;
        }
      };
      
      globalThis.Response = class {
        constructor(body, init) {
          this.body = body;
          this.status = (init && init.status) || 200;
          this.statusText = (init && init.statusText) || 'OK';
          this.headers = new Headers(init && init.headers);
          this.ok = this.status >= 200 && this.status < 300;
        }
        async json() {
          return JSON.parse(this.body);
        }
        async text() {
          return String(this.body);
        }
      };
      
      console.log("[V8] Browser polyfills initialized");
    }
    "#;

    // Execute the bundled Stream Chat code
    let js_code = format!(
      r#"
      {}
      
      {}
      
      console.log("[V8] Loading Stream Chat bundle...");

      // Load the Bun-bundled Stream Chat SDK
      {}

      console.log("[V8] Bundle loaded, checking globals...");
      console.log("[V8] processStreamChatRequestSync available:", typeof processStreamChatRequestSync);
      console.log("[V8] renderStreamChatHTML available:", typeof renderStreamChatHTML);
      console.log("[V8] css available:", typeof css);

      // Synchronous execution wrapper
      try {{
          console.log("[V8] Starting execution for action: {} with user: {}");

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
                        <p><strong>Bundle Path:</strong> client/dist/v8/stream-chat-demo.js</p>
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
      console_setup,
      browser_polyfills,
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

    // Extract and print console output from JavaScript
    let console_output_js = v8::String::new(scope, "globalThis._consoleOutput || []").unwrap();
    let console_script = v8::Script::compile(scope, console_output_js, None).unwrap();
    if let Some(console_result) = console_script.run(scope) {
      if let Ok(console_array) = v8::Local::<v8::Array>::try_from(console_result) {
        let length = console_array.length();
        println!("🔍 JavaScript Console Output ({} messages):", length);
        for i in 0 .. length {
          if let Some(element) = console_array.get_index(scope, i) {
            if let Some(element_str) = element.to_string(scope) {
              let message = element_str.to_rust_string_lossy(scope);
              println!("  {}", message);
            }
          }
        }
        if length == 0 {
          println!("  (No console output captured)");
        }
      }
    }

    println!("rust_string: {:?}", rust_string);
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
