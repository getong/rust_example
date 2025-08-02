use std::{cell::RefCell, fs::read_to_string, time::Instant};

use axum::{Router, extract::Query, response::Html, routing::get};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssr_rs::Ssr;

mod config;
mod real_v8_executor;
mod simple_v8_executor;
mod v8_processor;
mod v8_stream_executor;

thread_local! {
    static SSR: RefCell<Ssr<'static, 'static>> = RefCell::new({
        let js_code = match read_to_string("client/dist/ssr/index.js") {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Failed to read SSR file: {}", e);
                eprintln!("Current directory: {:?}", std::env::current_dir());
                panic!("Cannot initialize SSR without the JS bundle");
            }
        };
        let polyfill = r#"
// Polyfills for V8 environment
if (typeof MessageChannel === 'undefined') {
    globalThis.MessageChannel = function() {
        const channel = {};
        channel.port1 = { postMessage: function() {}, onmessage: null };
        channel.port2 = { postMessage: function() {}, onmessage: null };
        return channel;
    };
}

// Mock fetch for demonstration (in real scenarios, you'd use a proper fetch polyfill)
if (typeof fetch === 'undefined') {
    globalThis.fetch = async function(url) {
        return {
            ok: true,
            status: 200,
            json: async () => ({
                url: url,
                method: 'GET',
                timestamp: new Date().toISOString(),
                message: 'Mock response from V8 environment',
                data: { users: [{ id: 1, name: 'John' }, { id: 2, name: 'Jane' }] }
            })
        };
    };
}
"#;
        let enhanced_js = format!("{}\n{}", polyfill, js_code);
        match Ssr::from(enhanced_js, "SSR") {
            Ok(ssr) => ssr,
            Err(e) => {
                eprintln!("Failed to initialize SSR: {}", e);
                panic!("Cannot create SSR instance");
            }
        }
    })
}

#[derive(Deserialize)]
struct QueryParams {
  demo: Option<String>,
  #[allow(dead_code)]
  data: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct WeatherData {
  city: String,
  temperature: i32,
  humidity: i32,
  conditions: String,
  wind: WindData,
  forecast: Vec<ForecastDay>,
  timestamp: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct WindData {
  speed: i32,
  direction: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ForecastDay {
  day: String,
  high: i32,
  low: i32,
  condition: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct UserProfile {
  id: i32,
  username: String,
  email: String,
  profile: ProfileData,
  preferences: UserPreferences,
  stats: UserStats,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProfileData {
  #[serde(rename = "firstName")]
  first_name: String,
  #[serde(rename = "lastName")]
  last_name: String,
  avatar: String,
  bio: String,
  location: String,
  #[serde(rename = "joinDate")]
  join_date: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct UserPreferences {
  theme: String,
  language: String,
  notifications: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct UserStats {
  #[serde(rename = "projectsCreated")]
  projects_created: i32,
  #[serde(rename = "linesOfCode")]
  lines_of_code: i32,
  #[serde(rename = "contributionsThisYear")]
  contributions_this_year: i32,
}

#[tokio::main]
async fn main() {
  // ssr_rs will handle V8 platform initialization
  Ssr::create_platform();

  // Initialize V8 for real Stream Chat execution
  if let Err(e) = real_v8_executor::RealV8Executor::initialize() {
    eprintln!("❌ Failed to initialize V8 executor: {}", e);
    std::process::exit(1);
  }
  println!("✅ Server starting with Stream Chat V8 support");

  // build our application with multiple demonstration routes
  let app = Router::new()
    .route("/", get(root))
    .route("/test", get(test_route))
    .route("/calc", get(calc_demo))
    .route("/fetch", get(fetch_demo))
    .route("/data", get(data_demo))
    .route("/time", get(time_demo))
    .route("/weather", get(weather_demo))
    .route("/profile", get(profile_demo))
    .route("/system", get(system_demo))
    .route("/business", get(business_demo))
    .route("/dashboard", get(dashboard_demo))
    .route("/v8", get(v8_demo_safe))
    .route("/v8/typescript", get(v8_typescript_demo))
    .route("/v8/jsonplaceholder", get(v8_jsonplaceholder_demo))
    .route("/stream-chat", get(stream_chat_demo))
    .route("/stream-chat/authenticate", get(stream_chat_authenticate))
    .route("/stream-chat/user-context", get(stream_chat_user_context))
    .route("/stream-chat/analytics", get(stream_chat_analytics))
    .route("/stream-chat/setup", get(stream_chat_setup))
    .route("/stream-chat/token", get(stream_chat_token_demo));

  // run our app with hyper, listening globally on port 3000
  let listener = match tokio::net::TcpListener::bind("0.0.0.0:8080").await {
    Ok(listener) => listener,
    Err(e) => {
      eprintln!("Failed to bind to port 8080: {}", e);
      return;
    }
  };
  println!("Server running on http://0.0.0.0:8080");
  if let Err(e) = axum::serve(listener, app).await {
    eprintln!("Server error: {}", e);
  }
}

async fn root() -> Html<String> {
  render_page("Index", None, "Basic SSR Demo")
}

// Test route to verify ssr_rs function calling
async fn test_route() -> Html<String> {
  let result = SSR.with(|ssr| ssr.borrow_mut().render_to_string(Some("test")));

  match result {
    Ok(test_result) => Html(format!(
      "<html><body><h1>Test Result</h1><p>TypeScript returned: {}</p></body></html>",
      test_result
    )),
    Err(e) => Html(format!(
      "<html><body><h1>Test Error</h1><p>Error: {:?}</p></body></html>",
      e
    )),
  }
}

// Demonstrate calling TypeScript calculation function from Rust
async fn calc_demo(Query(_params): Query<QueryParams>) -> Html<String> {
  let start = Instant::now();

  // Call TypeScript calculate function from Rust
  let result = SSR.with(|ssr| {
    let mut ssr_instance = ssr.borrow_mut();
    ssr_instance.render_to_string(Some("calculate"))
  });

  match result {
    Ok(calc_result) => {
      println!("Calculation result from TypeScript: {}", calc_result);
      println!("Calc elapsed: {:?}", start.elapsed());

      // Since ssr_rs returns HTML, let's demonstrate with actual calculation
      let a = 15;
      let b = 8;
      let result = a * b;

      let calc_html = format!(
        r#"
        <div class="calc-result">
          <h2>🧮 Calculation Demo</h2>
          <div class="calculation">
            <h3>Mathematical Operation</h3>
            <div class="equation">
              <span class="number">{}</span>
              <span class="operator">×</span>
              <span class="number">{}</span>
              <span class="equals">=</span>
              <span class="result">{}</span>
            </div>
            <div class="metadata">
              <p><strong>Computed by:</strong> Rust Backend</p>
              <p><strong>Processing Time:</strong> {:?}</p>
              <p><strong>Timestamp:</strong> {}</p>
            </div>
          </div>
          <div class="additional-calcs">
            <h3>Additional Calculations</h3>
            <ul>
              <li>{} + {} = {}</li>
              <li>{} - {} = {}</li>
              <li>{} ÷ {} = {:.2}</li>
              <li>{}<sup>2</sup> = {}</li>
              <li>√{} = {:.2}</li>
            </ul>
          </div>
        </div>
        <style>
          .calc-result {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
          .calculation {{ background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
          .equation {{ font-size: 2em; text-align: center; margin: 20px 0; }}
          .number {{ color: #007acc; font-weight: bold; }}
          .operator {{ margin: 0 15px; color: #666; }}
          .equals {{ margin: 0 15px; }}
          .result {{ color: #28a745; font-weight: bold; }}
          .metadata {{ margin-top: 20px; padding-top: 20px; border-top: 1px solid #eee; }}
          .metadata p {{ margin: 5px 0; }}
          .additional-calcs {{ background: white; padding: 20px; border-radius: 5px; }}
          .additional-calcs ul {{ list-style: none; padding: 0; }}
          .additional-calcs li {{ padding: 5px 0; font-family: monospace; }}
        </style>
        "#,
        a,
        b,
        result,
        start.elapsed(),
        chrono::Utc::now().to_rfc3339(),
        a,
        b,
        a + b,
        a,
        b,
        a - b,
        a,
        b,
        a as f64 / b as f64,
        a,
        a * a,
        result,
        (result as f64).sqrt()
      );

      render_custom_html(&calc_html, "Rust Calculation Demo")
    }
    Err(e) => {
      eprintln!("Calculation Error: {}", e);
      let error_html = format!("<div class='error'>Calculation failed: {}</div>", e);
      render_custom_html(&error_html, "Calculation Error")
    }
  }
}

// Demonstrate calling TypeScript fetch function from Rust
async fn fetch_demo(Query(params): Query<QueryParams>) -> Html<String> {
  let start = Instant::now();

  // Determine which TypeScript function to call based on query parameter
  let api_type = params.demo.as_deref().unwrap_or("users");
  let ts_function = match api_type {
    "weather" => "getWeatherData",
    "business" => "processBusinessLogic",
    "products" => "processBusinessLogic", // We'll reuse this for products demo
    "system" => "getSystemInfo",
    _ => "getUserProfile", // Default to user profile
  };

  // Call the appropriate TypeScript function from Rust
  let result = SSR.with(|ssr| {
    let mut ssr_instance = ssr.borrow_mut();
    ssr_instance.render_to_string(Some(ts_function))
  });

  match result {
    Ok(fetch_result) => {
      println!(
        "Fetch result from TypeScript ({}): {}",
        ts_function, fetch_result
      );
      println!("Fetch elapsed: {:?}", start.elapsed());

      // Try to extract JSON data from the HTML result
      let decoded_result = if fetch_result.contains("<div") || fetch_result.contains("<link") {
        // It's HTML, try to extract JSON from data attributes or comments
        if let Some(data_start) = fetch_result.find("data-json=\"") {
          let data_start = data_start + "data-json=\"".len();
          if let Some(data_end) = fetch_result[data_start ..].find("\"") {
            let json_str = &fetch_result[data_start .. data_start + data_end]
              .replace("&quot;", "\"")
              .replace("&amp;", "&")
              .replace("&lt;", "<")
              .replace("&gt;", ">");
            match serde_json::from_str::<serde_json::Value>(json_str) {
              Ok(json_data) => {
                println!(
                  "Successfully extracted JSON from HTML data attribute: {:?}",
                  json_data
                );
                Some(json_data)
              }
              Err(e) => {
                println!("Failed to parse JSON from data attribute: {}", e);
                None
              }
            }
          } else {
            None
          }
        } else if fetch_result.contains("{") {
          // Try to extract JSON from within the HTML
          if let Some(json_start) = fetch_result.find('{') {
            if let Some(json_end) = fetch_result.rfind('}') {
              let json_str = &fetch_result[json_start ..= json_end];
              match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(json_data) => {
                  println!(
                    "Successfully extracted JSON from HTML content: {:?}",
                    json_data
                  );
                  Some(json_data)
                }
                Err(e) => {
                  println!("Failed to parse extracted JSON: {}", e);
                  None
                }
              }
            } else {
              None
            }
          } else {
            None
          }
        } else {
          println!("HTML result contains no JSON data");
          None
        }
      } else if fetch_result.trim().starts_with('"') && fetch_result.trim().ends_with('"') {
        // It's a JSON string, remove quotes and parse
        let json_str = fetch_result.trim().trim_matches('"').replace("\\\"", "\"");
        match serde_json::from_str::<serde_json::Value>(&json_str) {
          Ok(json_data) => {
            println!(
              "Successfully decoded JSON string from TypeScript: {:?}",
              json_data
            );
            Some(json_data)
          }
          Err(e) => {
            println!("Failed to decode JSON string from TypeScript: {}", e);
            None
          }
        }
      } else if fetch_result.trim().starts_with('{') || fetch_result.trim().starts_with('[') {
        // It looks like direct JSON, try to parse it
        match serde_json::from_str::<serde_json::Value>(&fetch_result) {
          Ok(json_data) => {
            println!("Successfully parsed direct JSON: {:?}", json_data);
            Some(json_data)
          }
          Err(e) => {
            println!("Failed to parse direct JSON: {}", e);
            None
          }
        }
      } else {
        println!("Result doesn't appear to be JSON: {}", fetch_result);
        None
      };

      // Create data structure using decoded JSON from TypeScript or fallback
      let fetch_data = match (api_type, &decoded_result) {
        ("weather", Some(json)) => {
          serde_json::json!({
            "url": "TypeScript: getWeatherData()",
            "method": "SSR Function Call",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "Data from TypeScript getWeatherData function",
            "ts_function": ts_function,
            "ts_result": json,
            "data": {
              "location": json.get("city").and_then(|v| v.as_str()).unwrap_or("Unknown"),
              "current": {
                "temperature": json.get("temperature").and_then(|v| v.as_i64()).unwrap_or(0),
                "humidity": json.get("humidity").and_then(|v| v.as_i64()).unwrap_or(0),
                "conditions": json.get("conditions").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "wind_speed": json.get("wind").and_then(|w| w.get("speed")).and_then(|v| v.as_i64()).unwrap_or(0),
                "wind_direction": json.get("wind").and_then(|w| w.get("direction")).and_then(|v| v.as_str()).unwrap_or("Unknown")
              },
              "forecast": json.get("forecast").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().map(|day| serde_json::json!({
                  "day": day.get("day").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                  "high": day.get("high").and_then(|v| v.as_i64()).unwrap_or(0),
                  "low": day.get("low").and_then(|v| v.as_i64()).unwrap_or(0),
                  "conditions": day.get("condition").and_then(|v| v.as_str()).unwrap_or("Unknown")
                })).collect::<Vec<_>>()
              }).unwrap_or_else(|| vec![])
            },
            "processing_time": format!("{:?}", start.elapsed())
          })
        }
        ("products", Some(json)) => {
          // Use processBusinessLogic result and transform to products format
          serde_json::json!({
            "url": "TypeScript: processBusinessLogic()",
            "method": "SSR Function Call",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "Business data from TypeScript transformed to products format",
            "ts_function": ts_function,
            "ts_result": json,
            "data": {
              "products": [
                {
                  "id": 201,
                  "name": "Business Analytics Software",
                  "price": json.get("summary").and_then(|s| s.get("totalRevenue")).and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0,
                  "category": "Business",
                  "stock": json.get("summary").and_then(|s| s.get("profit")).and_then(|v| v.as_i64()).unwrap_or(0) / 100,
                  "rating": 4.8
                },
                {
                  "id": 202,
                  "name": "Revenue Tracker",
                  "price": json.get("summary").and_then(|s| s.get("totalExpenses")).and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0,
                  "category": "Business",
                  "stock": 25,
                  "rating": 4.5
                }
              ],
              "total": 2,
              "categories": ["Business", "Analytics"],
              "period": json.get("period").and_then(|v| v.as_str()).unwrap_or("Unknown")
            },
            "processing_time": format!("{:?}", start.elapsed())
          })
        }
        ("system", Some(json)) => {
          serde_json::json!({
            "url": "TypeScript: getSystemInfo()",
            "method": "SSR Function Call",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "System info from TypeScript function",
            "ts_function": ts_function,
            "ts_result": json,
            "data": {
              "system": {
                "runtime": json.get("runtime").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "platform": json.get("platform").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "memory_used": json.get("memory").and_then(|m| m.get("used")).and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "features": json.get("features").and_then(|v| v.as_array()).unwrap_or(&vec![]).iter()
                  .filter_map(|f| f.as_str()).collect::<Vec<_>>()
              }
            },
            "processing_time": format!("{:?}", start.elapsed())
          })
        }
        (_, Some(json)) => {
          // Default: Use decoded JSON as user profile
          serde_json::json!({
            "url": "TypeScript: getUserProfile()",
            "method": "SSR Function Call",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "User profile from TypeScript function",
            "ts_function": ts_function,
            "ts_result": json,
            "data": {
              "user": {
                "id": json.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "username": json.get("username").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "email": json.get("email").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "name": format!("{} {}",
                  json.get("profile").and_then(|p| p.get("firstName")).and_then(|v| v.as_str()).unwrap_or(""),
                  json.get("profile").and_then(|p| p.get("lastName")).and_then(|v| v.as_str()).unwrap_or("")
                ),
                "role": json.get("profile").and_then(|p| p.get("bio")).and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "projects": json.get("stats").and_then(|s| s.get("projectsCreated")).and_then(|v| v.as_i64()).unwrap_or(0)
              }
            },
            "processing_time": format!("{:?}", start.elapsed())
          })
        }
        _ => {
          // Fallback when no JSON could be decoded
          serde_json::json!({
            "url": format!("TypeScript: {}()", ts_function),
            "method": "SSR Function Call (Fallback)",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "Could not decode JSON from TypeScript function",
            "ts_function": ts_function,
            "raw_result": fetch_result,
            "data": {
              "error": "Failed to decode JSON result from TypeScript function"
            },
            "processing_time": format!("{:?}", start.elapsed())
          })
        }
      };

      // Render different content based on the API type
      let fetch_html = if fetch_data["url"].as_str().unwrap_or("").contains("weather") {
        // Weather API response
        let current = &fetch_data["data"]["current"];
        let empty_vec = vec![];
        let forecast = fetch_data["data"]["forecast"]
          .as_array()
          .unwrap_or(&empty_vec);

        format!(
          r#"
          <div class="fetch-result">
            <h2>🌐 Fetch API Result - Weather Data</h2>
            <div class="fetch-info">
              <p><strong>TypeScript Function:</strong> {}</p>
              <p><strong>Method:</strong> {}</p>
              <p><strong>Timestamp:</strong> {}</p>
              <p><strong>Processing Time:</strong> {}</p>
            </div>
            <div class="ts-result-info">
              <h4>Raw TypeScript Result:</h4>
              <pre class="json-result">{}</pre>
            </div>
            <div class="fetch-data">
              <h3>Processed Data: {}</h3>
              <div class="weather-response">
                <h4>📍 {}</h4>
                <div class="current-weather">
                  <div class="weather-stat">
                    <span class="label">Temperature:</span>
                    <span class="value">{}°F</span>
                  </div>
                  <div class="weather-stat">
                    <span class="label">Humidity:</span>
                    <span class="value">{}%</span>
                  </div>
                  <div class="weather-stat">
                    <span class="label">Conditions:</span>
                    <span class="value">{}</span>
                  </div>
                  <div class="weather-stat">
                    <span class="label">Wind:</span>
                    <span class="value">{} mph {}</span>
                  </div>
                </div>
                <h4>3-Day Forecast</h4>
                <div class="forecast">
                  {}
                </div>
              </div>
            </div>
            <div class="api-examples">
              <h4>Try different API examples:</h4>
              <a href="/fetch?demo=users" class="api-link">Users API</a>
              <a href="/fetch?demo=weather" class="api-link active">Weather API</a>
              <a href="/fetch?demo=products" class="api-link">Products API</a>
            </div>
          </div>
          <style>
            .fetch-result {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
            .fetch-info {{ background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
            .fetch-info p {{ margin: 5px 0; }}
            .fetch-data {{ background: white; padding: 15px; border-radius: 5px; }}
            .fetch-data h3 {{ color: #007acc; margin-top: 15px; }}
            .weather-response {{ margin-top: 15px; }}
            .current-weather {{ display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin: 15px 0; }}
            .weather-stat {{ display: flex; justify-content: space-between; padding: 8px; background: #f0f0f0; border-radius: 4px; }}
            .weather-stat .label {{ font-weight: bold; }}
            .weather-stat .value {{ color: #007acc; }}
            .forecast {{ display: flex; gap: 15px; }}
            .forecast-day {{ flex: 1; background: #f0f0f0; padding: 10px; border-radius: 5px; text-align: center; }}
            .ts-result-info {{ background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
            .json-result {{ background: #f8f9fa; border: 1px solid #e9ecef; border-radius: 4px; padding: 10px; font-size: 0.85em; overflow-x: auto; max-height: 200px; overflow-y: auto; white-space: pre-wrap; }}
            .api-examples {{ margin-top: 20px; padding: 15px; background: white; border-radius: 5px; }}
            .api-link {{ display: inline-block; margin: 5px; padding: 8px 15px; background: #f0f0f0; border-radius: 5px; text-decoration: none; color: #333; }}
            .api-link.active {{ background: #007acc; color: white; }}
            .api-link:hover {{ background: #e0e0e0; }}
          </style>
          "#,
          fetch_data["ts_function"]
            .as_str()
            .unwrap_or(fetch_data["url"].as_str().unwrap_or("")),
          fetch_data["method"].as_str().unwrap_or(""),
          fetch_data["timestamp"].as_str().unwrap_or(""),
          fetch_data["processing_time"].as_str().unwrap_or(""),
          serde_json::to_string_pretty(
            fetch_data
              .get("ts_result")
              .unwrap_or(&serde_json::Value::Null)
          )
          .unwrap_or_else(|_| "No TypeScript result".to_string()),
          fetch_data["message"].as_str().unwrap_or(""),
          fetch_data["data"]["location"].as_str().unwrap_or(""),
          current["temperature"].as_i64().unwrap_or(0),
          current["humidity"].as_i64().unwrap_or(0),
          current["conditions"].as_str().unwrap_or(""),
          current["wind_speed"].as_i64().unwrap_or(0),
          current["wind_direction"].as_str().unwrap_or(""),
          forecast
            .iter()
            .map(|day| format!(
              "<div class='forecast-day'><strong>{}</strong><br/>High: {}°F<br/>Low: \
               {}°F<br/>{}</div>",
              day["day"].as_str().unwrap_or(""),
              day["high"].as_i64().unwrap_or(0),
              day["low"].as_i64().unwrap_or(0),
              day["conditions"].as_str().unwrap_or("")
            ))
            .collect::<Vec<_>>()
            .join("")
        )
      } else if fetch_data["url"]
        .as_str()
        .unwrap_or("")
        .contains("products")
      {
        // Products API response
        let empty_vec = vec![];
        let products = fetch_data["data"]["products"]
          .as_array()
          .unwrap_or(&empty_vec);

        format!(
          r#"
          <div class="fetch-result">
            <h2>🌐 Fetch API Result - Product Catalog</h2>
            <div class="fetch-info">
              <p><strong>URL:</strong> {}</p>
              <p><strong>Method:</strong> {}</p>
              <p><strong>Timestamp:</strong> {}</p>
              <p><strong>Processing Time:</strong> {}</p>
            </div>
            <div class="fetch-data">
              <h3>Response: {}</h3>
              <h4>Products ({} total)</h4>
              <div class="product-grid">
                {}
              </div>
              <div class="categories">
                <h4>Available Categories</h4>
                <div class="category-list">
                  {}
                </div>
              </div>
            </div>
            <div class="api-examples">
              <h4>Try different API examples:</h4>
              <a href="/fetch?demo=users" class="api-link">Users API</a>
              <a href="/fetch?demo=weather" class="api-link">Weather API</a>
              <a href="/fetch?demo=products" class="api-link active">Products API</a>
            </div>
          </div>
          <style>
            .fetch-result {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
            .fetch-info {{ background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
            .fetch-info p {{ margin: 5px 0; }}
            .fetch-data {{ background: white; padding: 15px; border-radius: 5px; }}
            .fetch-data h3 {{ color: #007acc; margin-top: 15px; }}
            .fetch-data h4 {{ margin-top: 15px; }}
            .product-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 20px; margin-top: 15px; }}
            .product-card {{ border: 1px solid #ddd; border-radius: 8px; padding: 15px; background: #f9f9f9; }}
            .product-name {{ font-weight: bold; font-size: 1.1em; margin-bottom: 8px; }}
            .product-price {{ color: #28a745; font-size: 1.2em; font-weight: bold; }}
            .product-category {{ color: #666; font-size: 0.9em; }}
            .product-stock {{ margin-top: 8px; }}
            .stock-low {{ color: #dc3545; }}
            .stock-high {{ color: #28a745; }}
            .product-rating {{ color: #ffc107; margin-top: 5px; }}
            .categories {{ margin-top: 20px; }}
            .category-list {{ display: flex; gap: 10px; margin-top: 10px; }}
            .category-tag {{ background: #007acc; color: white; padding: 5px 15px; border-radius: 20px; font-size: 0.9em; }}
            .api-examples {{ margin-top: 20px; padding: 15px; background: white; border-radius: 5px; }}
            .api-link {{ display: inline-block; margin: 5px; padding: 8px 15px; background: #f0f0f0; border-radius: 5px; text-decoration: none; color: #333; }}
            .api-link.active {{ background: #007acc; color: white; }}
            .api-link:hover {{ background: #e0e0e0; }}
          </style>
          "#,
          fetch_data["url"].as_str().unwrap_or(""),
          fetch_data["method"].as_str().unwrap_or(""),
          fetch_data["timestamp"].as_str().unwrap_or(""),
          fetch_data["processing_time"].as_str().unwrap_or(""),
          fetch_data["message"].as_str().unwrap_or(""),
          products.len(),
          products
            .iter()
            .map(|product| {
              let stock = product["stock"].as_i64().unwrap_or(0);
              let stock_class = if stock < 20 {
                "stock-low"
              } else {
                "stock-high"
              };
              let rating = product["rating"].as_f64().unwrap_or(0.0);
              format!(
                r#"<div class='product-card'>
                  <div class='product-name'>{}</div>
                  <div class='product-price'>${}</div>
                  <div class='product-category'>Category: {}</div>
                  <div class='product-stock {}'>{} in stock</div>
                  <div class='product-rating'>★ {} / 5.0</div>
                </div>"#,
                product["name"].as_str().unwrap_or(""),
                product["price"].as_f64().unwrap_or(0.0),
                product["category"].as_str().unwrap_or(""),
                stock_class,
                stock,
                rating
              )
            })
            .collect::<Vec<_>>()
            .join(""),
          fetch_data["data"]["categories"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|cat| format!(
              "<span class='category-tag'>{}</span>",
              cat.as_str().unwrap_or("")
            ))
            .collect::<Vec<_>>()
            .join("")
        )
      } else {
        // Users API response
        format!(
          r#"
          <div class="fetch-result">
            <h2>🌐 Fetch API Result - User Management</h2>
            <div class="fetch-info">
              <p><strong>URL:</strong> {}</p>
              <p><strong>Method:</strong> {}</p>
              <p><strong>Timestamp:</strong> {}</p>
              <p><strong>Processing Time:</strong> {}</p>
            </div>
            <div class="fetch-data">
              <h3>Response: {}</h3>
              <h4>Users ({} total)</h4>
              <table style="width: 100%; border-collapse: collapse;">
                <thead>
                  <tr style="background: #f0f0f0;">
                    <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">ID</th>
                    <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">Name</th>
                    <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">Role</th>
                    <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">Department</th>
                  </tr>
                </thead>
                <tbody>
                  {}
                </tbody>
              </table>
            </div>
            <div class="api-examples">
              <h4>Try different API examples:</h4>
              <a href="/fetch?demo=users" class="api-link active">Users API</a>
              <a href="/fetch?demo=weather" class="api-link">Weather API</a>
              <a href="/fetch?demo=products" class="api-link">Products API</a>
            </div>
          </div>
          <style>
            .fetch-result {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
            .fetch-info {{ background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
            .fetch-info p {{ margin: 5px 0; }}
            .fetch-data {{ background: white; padding: 15px; border-radius: 5px; }}
            .fetch-data h3 {{ color: #007acc; margin-top: 15px; }}
            .fetch-data h4 {{ margin-top: 15px; }}
            .api-examples {{ margin-top: 20px; padding: 15px; background: white; border-radius: 5px; }}
            .api-link {{ display: inline-block; margin: 5px; padding: 8px 15px; background: #f0f0f0; border-radius: 5px; text-decoration: none; color: #333; }}
            .api-link.active {{ background: #007acc; color: white; }}
            .api-link:hover {{ background: #e0e0e0; }}
          </style>
          "#,
          fetch_data["url"].as_str().unwrap_or(""),
          fetch_data["method"].as_str().unwrap_or(""),
          fetch_data["timestamp"].as_str().unwrap_or(""),
          fetch_data["processing_time"].as_str().unwrap_or(""),
          fetch_data["message"].as_str().unwrap_or(""),
          fetch_data["data"]["total"].as_i64().unwrap_or(0),
          fetch_data["data"]["users"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|user| format!(
              "<tr><td style='padding: 8px; border: 1px solid #ddd;'>{}</td><td style='padding: \
               8px; border: 1px solid #ddd;'>{}</td><td style='padding: 8px; border: 1px solid \
               #ddd;'>{}</td><td style='padding: 8px; border: 1px solid #ddd;'>{}</td></tr>",
              user["id"].as_i64().unwrap_or(0),
              user["name"].as_str().unwrap_or(""),
              user["role"].as_str().unwrap_or(""),
              user["department"].as_str().unwrap_or("")
            ))
            .collect::<Vec<_>>()
            .join("")
        )
      };

      render_custom_html(&fetch_html, "Fetch API Demonstration")
    }
    Err(e) => {
      eprintln!("Fetch Error: {}", e);
      let error_html = format!("<div class='error'>Fetch failed: {}</div>", e);
      render_custom_html(&error_html, "Fetch Error")
    }
  }
}

// Demonstrate passing complex data from Rust to TypeScript
async fn data_demo(Query(_params): Query<QueryParams>) -> Html<String> {
  let users = vec![
    serde_json::json!({"id": 1, "name": "Alice", "role": "Admin", "status": "Active", "last_login": "2024-01-15"}),
    serde_json::json!({"id": 2, "name": "Bob", "role": "User", "status": "Active", "last_login": "2024-01-14"}),
    serde_json::json!({"id": 3, "name": "Charlie", "role": "Moderator", "status": "Inactive", "last_login": "2024-01-10"}),
    serde_json::json!({"id": 4, "name": "Diana", "role": "User", "status": "Active", "last_login": "2024-01-15"}),
    serde_json::json!({"id": 5, "name": "Eve", "role": "Admin", "status": "Active", "last_login": "2024-01-15"}),
  ];

  let data_html = format!(
    r#"
    <div class="data-demo">
      <h2>📊 Data Processing Demo</h2>
      <div class="data-overview">
        <h3>User Management System</h3>
        <div class="stats">
          <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Total Users</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Active Users</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Admins</div>
          </div>
        </div>
      </div>
      <div class="user-table">
        <h3>User List</h3>
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>Name</th>
              <th>Role</th>
              <th>Status</th>
              <th>Last Login</th>
            </tr>
          </thead>
          <tbody>
            {}
          </tbody>
        </table>
      </div>
      <div class="metadata">
        <p><strong>Generated by:</strong> Rust Backend</p>
        <p><strong>Timestamp:</strong> {}</p>
      </div>
    </div>
    <style>
      .data-demo {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
      .data-overview {{ background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
      .stats {{ display: flex; gap: 20px; margin-top: 15px; }}
      .stat-card {{ flex: 1; background: #007acc; color: white; padding: 20px; border-radius: 5px; text-align: center; }}
      .stat-value {{ font-size: 2em; font-weight: bold; }}
      .stat-label {{ margin-top: 5px; opacity: 0.9; }}
      .user-table {{ background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
      .user-table table {{ width: 100%; border-collapse: collapse; }}
      .user-table th, .user-table td {{ padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }}
      .user-table th {{ background: #f0f0f0; font-weight: bold; }}
      .metadata {{ background: white; padding: 15px; border-radius: 5px; }}
      .metadata p {{ margin: 5px 0; }}
    </style>
    "#,
    users.len(),
    users.iter().filter(|u| u["status"] == "Active").count(),
    users.iter().filter(|u| u["role"] == "Admin").count(),
    users
      .iter()
      .map(|user| format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td style='color: {};'>{}</td><td>{}</td></tr>",
        user["id"].as_i64().unwrap_or(0),
        user["name"].as_str().unwrap_or(""),
        user["role"].as_str().unwrap_or(""),
        if user["status"] == "Active" {
          "#28a745"
        } else {
          "#dc3545"
        },
        user["status"].as_str().unwrap_or(""),
        user["last_login"].as_str().unwrap_or("")
      ))
      .collect::<Vec<_>>()
      .join(""),
    chrono::Utc::now().to_rfc3339()
  );

  render_custom_html(&data_html, "Data Processing Demo")
}

// Demonstrate calling TypeScript utility function
async fn time_demo() -> Html<String> {
  let _start = Instant::now();
  let rust_utc = chrono::Utc::now();
  let rust_local = chrono::Local::now();

  let time_html = format!(
    r#"
    <div class="time-demo">
      <h2>🕐 Time Synchronization Demo</h2>
      <div class="time-display">
        <div class="clock-card">
          <h3>UTC Time</h3>
          <div class="time">{}</div>
          <div class="date">{}</div>
          <div class="timezone">Coordinated Universal Time</div>
        </div>
        <div class="clock-card">
          <h3>Local Server Time</h3>
          <div class="time">{}</div>
          <div class="date">{}</div>
          <div class="timezone">{}</div>
        </div>
      </div>
      <div class="time-details">
        <h3>Time Information</h3>
        <table>
          <tr><td>Unix Timestamp:</td><td>{}</td></tr>
          <tr><td>ISO 8601 Format:</td><td>{}</td></tr>
          <tr><td>RFC 3339 Format:</td><td>{}</td></tr>
          <tr><td>Day of Year:</td><td>{}</td></tr>
          <tr><td>Week of Year:</td><td>{}</td></tr>
          <tr><td>Processing Engine:</td><td>Rust chrono library</td></tr>
        </table>
      </div>
    </div>
    <style>
      .time-demo {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
      .time-display {{ display: flex; gap: 20px; margin-bottom: 20px; }}
      .clock-card {{ flex: 1; background: white; padding: 20px; border-radius: 5px; text-align: center; }}
      .clock-card h3 {{ color: #007acc; margin-bottom: 15px; }}
      .time {{ font-size: 2.5em; font-weight: bold; color: #333; font-family: monospace; }}
      .date {{ font-size: 1.2em; color: #666; margin: 10px 0; }}
      .timezone {{ color: #999; font-size: 0.9em; }}
      .time-details {{ background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
      .time-details table {{ width: 100%; }}
      .time-details td {{ padding: 8px; border-bottom: 1px solid #eee; }}
      .time-details td:first-child {{ font-weight: bold; width: 40%; }}
    </style>
    "#,
    rust_utc.format("%H:%M:%S"),
    rust_utc.format("%Y-%m-%d"),
    rust_local.format("%H:%M:%S"),
    rust_local.format("%Y-%m-%d"),
    rust_local.format("%Z"),
    rust_utc.timestamp(),
    rust_utc.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    rust_utc.to_rfc3339(),
    rust_utc.ordinal(),
    rust_utc.iso_week().week()
  );

  render_custom_html(&time_html, "Time Synchronization Demo")
}

// Helper function to render pages with consistent structure
fn render_page(function_name: &str, data: Option<String>, title: &str) -> Html<String> {
  let start = Instant::now();

  let result = SSR.with(|ssr| {
    let mut ssr_instance = ssr.borrow_mut();
    match (function_name, &data) {
      ("renderWithData", Some(_data_str)) => {
        // First set the data, then render
        let _set_result = ssr_instance.render_to_string(Some("setRustData"));
        // For now, render without data since we need a different approach
        ssr_instance.render_to_string(Some("renderWithData"))
      }
      _ => {
        // For simple functions
        ssr_instance.render_to_string(Some(function_name))
      }
    }
  });

  println!("SSR elapsed: {:?}", start.elapsed());

  match result {
    Ok(html) => {
      let navigation = r#"
            <nav style="padding: 10px; background: #f0f0f0; margin-bottom: 20px; font-size: 14px;">
                <a href="/" style="margin-right: 8px;">Home</a> |
                <a href="/calc" style="margin: 0 8px;">Calculator</a> |
                <a href="/fetch" style="margin: 0 8px;">Fetch</a> |
                <a href="/data" style="margin: 0 8px;">Data</a> |
                <a href="/time" style="margin: 0 8px;">Time</a> |
                <a href="/weather" style="margin: 0 8px;">Weather</a> |
                <a href="/profile" style="margin: 0 8px;">Profile</a> |
                <a href="/system" style="margin: 0 8px;">System</a> |
                <a href="/business" style="margin: 0 8px;">Business</a> |
                <a href="/dashboard" style="margin: 0 8px;">Dashboard</a> |
                <a href="/v8" style="margin: 0 8px;">V8 Demo</a> |
                <a href="/v8/typescript" style="margin: 0 8px;">V8 TypeScript</a> |
                <a href="/v8/jsonplaceholder" style="margin: 0 8px;">JSONPlaceholder</a> |
                <a href="/stream-chat" style="margin-left: 8px;">Stream Chat</a>
            </nav>
            "#;

      let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
</head>
<body>
    {}
    <div id="root">{}</div>
    <footer style="margin-top: 20px; padding: 10px; background: #f9f9f9; text-align: center;">
        <small>Powered by ssr_rs • Rust ↔ TypeScript Integration Demo</small>
    </footer>
</body>
</html>"#,
        title, navigation, html
      );
      Html(full_html)
    }
    Err(e) => {
      eprintln!("SSR Error: {}", e);
      Html(format!(
        "<html><body><h1>SSR Error</h1><p>Failed to render: {}</p></body></html>",
        e
      ))
    }
  }
}

// DEMONSTRATION: JS → Rust → HTML Processing Routes

// Weather Demo: Rust generates data → Processes → HTML generation
async fn weather_demo() -> Html<String> {
  let title = "Weather Dashboard (Rust Data → HTML)";

  // Since ssr_rs doesn't support calling custom functions reliably,
  // let's generate the data in Rust and demonstrate the HTML generation
  let weather = WeatherData {
    city: "San Francisco".to_string(),
    temperature: 72,
    humidity: 65,
    conditions: "Partly Cloudy".to_string(),
    wind: WindData {
      speed: 8,
      direction: "NW".to_string(),
    },
    forecast: vec![
      ForecastDay {
        day: "Today".to_string(),
        high: 75,
        low: 62,
        condition: "Sunny".to_string(),
      },
      ForecastDay {
        day: "Tomorrow".to_string(),
        high: 73,
        low: 60,
        condition: "Cloudy".to_string(),
      },
      ForecastDay {
        day: "Thursday".to_string(),
        high: 70,
        low: 58,
        condition: "Rain".to_string(),
      },
    ],
    timestamp: chrono::Utc::now().to_rfc3339(),
  };

  println!("Generated weather data: {:?}", weather);
  let weather_html = generate_weather_html(&weather);
  render_custom_html(&weather_html, title)
}

// Profile Demo: Rust → HTML
async fn profile_demo() -> Html<String> {
  let title = "User Profile (Rust Data → HTML)";

  // Generate profile data in Rust
  let profile = UserProfile {
    id: 1001,
    username: "rustdev".to_string(),
    email: "dev@rust-ts-bridge.com".to_string(),
    profile: ProfileData {
      first_name: "John".to_string(),
      last_name: "Rustacean".to_string(),
      avatar: "https://avatars.example.com/rustdev.jpg".to_string(),
      bio: "Full-stack developer loving Rust and TypeScript integration".to_string(),
      location: "San Francisco, CA".to_string(),
      join_date: "2023-01-15T00:00:00Z".to_string(),
    },
    preferences: UserPreferences {
      theme: "dark".to_string(),
      language: "en-US".to_string(),
      notifications: true,
    },
    stats: UserStats {
      projects_created: 42,
      lines_of_code: 15420,
      contributions_this_year: 287,
    },
  };

  println!("Generated profile data: {:?}", profile);
  let profile_html = generate_profile_html(&profile);
  render_custom_html(&profile_html, title)
}

// System Demo: Rust → HTML
async fn system_demo() -> Html<String> {
  let title = "System Information (Rust Data → HTML)";

  // Generate system info in Rust
  let system_info = serde_json::json!({
      "runtime": "V8 JavaScript Engine",
      "platform": "Server-Side Rendering",
      "memory": {
          "used": "12.5 MB",
          "available": "2.1 GB"
      },
      "performance": {
          "renderTime": "3.2ms",
          "cacheHits": 89,
          "requestsHandled": 756
      },
      "features": [
          "React SSR",
          "TypeScript Integration",
          "Rust Backend",
          "V8 Engine",
          "Real-time Data"
      ]
  });

  println!("Generated system info: {:?}", system_info);
  let system_html = generate_system_html(&system_info);
  render_custom_html(&system_html, title)
}

// Business Demo: Rust → HTML
async fn business_demo() -> Html<String> {
  let title = "Business Analytics (Rust Data → HTML)";

  // Generate business data in Rust
  let sales_data = vec![
    serde_json::json!({ "month": "Jan", "revenue": 15000, "expenses": 8000 }),
    serde_json::json!({ "month": "Feb", "revenue": 18000, "expenses": 9500 }),
    serde_json::json!({ "month": "Mar", "revenue": 22000, "expenses": 11000 }),
    serde_json::json!({ "month": "Apr", "revenue": 25000, "expenses": 12500 }),
  ];

  let total_revenue: i64 = sales_data
    .iter()
    .map(|m| m["revenue"].as_i64().unwrap_or(0))
    .sum();
  let total_expenses: i64 = sales_data
    .iter()
    .map(|m| m["expenses"].as_i64().unwrap_or(0))
    .sum();
  let profit = total_revenue - total_expenses;
  let profit_margin = format!("{:.2}%", (profit as f64 / total_revenue as f64 * 100.0));

  let business_data = serde_json::json!({
      "period": "Q1 2024",
      "salesData": sales_data,
      "summary": {
          "totalRevenue": total_revenue,
          "totalExpenses": total_expenses,
          "profit": profit,
          "profitMargin": profit_margin,
          "averageMonthlyRevenue": total_revenue / 4,
          "growthRate": "18.5%"
      },
      "calculatedAt": chrono::Utc::now().to_rfc3339()
  });

  println!("Generated business data: {:?}", business_data);
  let business_html = generate_business_html(&business_data);
  render_custom_html(&business_html, title)
}

// Dashboard Demo: Rust → HTML (Combined Data)
async fn dashboard_demo() -> Html<String> {
  let title = "Complete Dashboard (Rust Data → HTML)";

  // Generate all data in Rust
  let weather = WeatherData {
    city: "San Francisco".to_string(),
    temperature: 72,
    humidity: 65,
    conditions: "Partly Cloudy".to_string(),
    wind: WindData {
      speed: 8,
      direction: "NW".to_string(),
    },
    forecast: vec![
      ForecastDay {
        day: "Today".to_string(),
        high: 75,
        low: 62,
        condition: "Sunny".to_string(),
      },
      ForecastDay {
        day: "Tomorrow".to_string(),
        high: 73,
        low: 60,
        condition: "Cloudy".to_string(),
      },
      ForecastDay {
        day: "Thursday".to_string(),
        high: 70,
        low: 58,
        condition: "Rain".to_string(),
      },
    ],
    timestamp: chrono::Utc::now().to_rfc3339(),
  };

  let profile = UserProfile {
    id: 1001,
    username: "rustdev".to_string(),
    email: "dev@rust-ts-bridge.com".to_string(),
    profile: ProfileData {
      first_name: "John".to_string(),
      last_name: "Rustacean".to_string(),
      avatar: "https://avatars.example.com/rustdev.jpg".to_string(),
      bio: "Full-stack developer loving Rust and TypeScript integration".to_string(),
      location: "San Francisco, CA".to_string(),
      join_date: "2023-01-15T00:00:00Z".to_string(),
    },
    preferences: UserPreferences {
      theme: "dark".to_string(),
      language: "en-US".to_string(),
      notifications: true,
    },
    stats: UserStats {
      projects_created: 42,
      lines_of_code: 15420,
      contributions_this_year: 287,
    },
  };

  let system_info = serde_json::json!({
      "runtime": "V8 JavaScript Engine",
      "platform": "Server-Side Rendering",
      "memory": {
          "used": "12.5 MB",
          "available": "2.1 GB"
      },
      "performance": {
          "renderTime": "3.2ms",
          "cacheHits": 89,
          "requestsHandled": 756
      },
      "features": [
          "React SSR",
          "TypeScript Integration",
          "Rust Backend",
          "V8 Engine",
          "Real-time Data"
      ]
  });

  let _sales_data = vec![
    serde_json::json!({ "month": "Jan", "revenue": 15000, "expenses": 8000 }),
    serde_json::json!({ "month": "Feb", "revenue": 18000, "expenses": 9500 }),
    serde_json::json!({ "month": "Mar", "revenue": 22000, "expenses": 11000 }),
    serde_json::json!({ "month": "Apr", "revenue": 25000, "expenses": 12500 }),
  ];

  let total_revenue: i64 = 80000;
  let total_expenses: i64 = 41000;
  let profit = 39000;

  let business_data = serde_json::json!({
      "summary": {
          "totalRevenue": total_revenue,
          "totalExpenses": total_expenses,
          "profit": profit,
      }
  });

  let mut dashboard_html = String::from("<div class='dashboard'>");
  dashboard_html.push_str("<h1>Complete Dashboard - Rust → HTML Integration</h1>");

  // Add all widgets
  dashboard_html.push_str(&format!(
    "<div class='widget'><h2>🌤️ Weather</h2>{}</div>",
    generate_weather_widget(&weather)
  ));
  dashboard_html.push_str(&format!(
    "<div class='widget'><h2>👤 Profile</h2>{}</div>",
    generate_profile_widget(&profile)
  ));
  dashboard_html.push_str(&format!(
    "<div class='widget'><h2>⚙️ System</h2>{}</div>",
    generate_system_widget(&system_info)
  ));
  dashboard_html.push_str(&format!(
    "<div class='widget'><h2>📊 Business</h2>{}</div>",
    generate_business_widget(&business_data)
  ));

  dashboard_html.push_str("</div>");

  // Add dashboard-specific CSS
  let dashboard_css = r#"
    <style>
        .dashboard { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; margin: 20px; }
        .widget { border: 1px solid #ddd; border-radius: 8px; padding: 15px; background: #fff; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .widget h2 { margin-top: 0; color: #333; border-bottom: 2px solid #007acc; padding-bottom: 5px; }
        .metric { display: flex; justify-content: space-between; margin: 8px 0; }
        .metric strong { color: #007acc; }
    </style>
    "#;

  render_custom_html_with_css(&dashboard_html, title, dashboard_css)
}

// HTML Generation Functions

fn generate_weather_html(weather: &WeatherData) -> String {
  format!(
    r#"
    <div class="weather-card">
        <h2>🌤️ Weather in {city}</h2>
        <div class="current">
            <div class="temp">{temperature}°F</div>
            <div class="condition">{conditions}</div>
            <div class="details">
                <p><strong>Humidity:</strong> {humidity}%</p>
                <p><strong>Wind:</strong> {wind_speed} mph {wind_direction}</p>
            </div>
        </div>
        <div class="forecast">
            <h3>3-Day Forecast</h3>
            {forecast_html}
        </div>
        <div class="timestamp">Updated: {timestamp}</div>
    </div>
    <style>
        .weather-card {{ background: linear-gradient(135deg, #74b9ff, #0984e3); color: white; padding: 20px; border-radius: 10px; }}
        .temp {{ font-size: 3em; font-weight: bold; }}
        .condition {{ font-size: 1.2em; margin: 10px 0; }}
        .forecast {{ display: flex; gap: 15px; margin: 15px 0; }}
        .forecast-day {{ background: rgba(255,255,255,0.2); padding: 10px; border-radius: 5px; text-align: center; }}
        .timestamp {{ font-size: 0.9em; opacity: 0.8; margin-top: 15px; }}
    </style>
    "#,
    city = weather.city,
    temperature = weather.temperature,
    conditions = weather.conditions,
    humidity = weather.humidity,
    wind_speed = weather.wind.speed,
    wind_direction = weather.wind.direction,
    forecast_html = weather
      .forecast
      .iter()
      .map(|day| format!(
        "<div class='forecast-day'><div>{}</div><div>{}°/{}</div><div>{}</div></div>",
        day.day, day.high, day.low, day.condition
      ))
      .collect::<Vec<_>>()
      .join(""),
    timestamp = chrono::DateTime::parse_from_rfc3339(&weather.timestamp)
      .unwrap_or_else(|_| chrono::Utc::now().into())
      .format("%Y-%m-%d %H:%M:%S")
  )
}

fn generate_profile_html(profile: &UserProfile) -> String {
  format!(
    r#"
    <div class="profile-card">
        <div class="profile-header">
            <img src="{avatar}" alt="Avatar" class="avatar">
            <div class="profile-info">
                <h2>{first_name} {last_name}</h2>
                <p class="username">@{username}</p>
                <p class="email">{email}</p>
            </div>
        </div>
        <div class="profile-details">
            <p class="bio">{bio}</p>
            <p><strong>📍 Location:</strong> {location}</p>
            <p><strong>📅 Joined:</strong> {join_date}</p>
        </div>
        <div class="profile-stats">
            <div class="stat">
                <strong>{projects_created}</strong>
                <span>Projects</span>
            </div>
            <div class="stat">
                <strong>{lines_of_code}</strong>
                <span>Lines of Code</span>
            </div>
            <div class="stat">
                <strong>{contributions_this_year}</strong>
                <span>Contributions</span>
            </div>
        </div>
        <div class="preferences">
            <p><strong>Preferences:</strong> {theme} theme, {language}, notifications {notifications}</p>
        </div>
    </div>
    <style>
        .profile-card {{ background: #fff; border: 1px solid #ddd; border-radius: 10px; padding: 20px; }}
        .profile-header {{ display: flex; align-items: center; margin-bottom: 20px; }}
        .avatar {{ width: 80px; height: 80px; border-radius: 50%; margin-right: 20px; background: #f0f0f0; }}
        .profile-stats {{ display: flex; justify-content: space-around; margin: 20px 0; }}
        .stat {{ text-align: center; }}
        .stat strong {{ display: block; font-size: 1.5em; color: #007acc; }}
        .bio {{ font-style: italic; margin: 15px 0; }}
        .username {{ color: #666; }}
    </style>
    "#,
    avatar = profile.profile.avatar,
    first_name = profile.profile.first_name,
    last_name = profile.profile.last_name,
    username = profile.username,
    email = profile.email,
    bio = profile.profile.bio,
    location = profile.profile.location,
    join_date = profile.profile.join_date,
    projects_created = profile.stats.projects_created,
    lines_of_code = profile.stats.lines_of_code,
    contributions_this_year = profile.stats.contributions_this_year,
    theme = profile.preferences.theme,
    language = profile.preferences.language,
    notifications = if profile.preferences.notifications {
      "enabled"
    } else {
      "disabled"
    }
  )
}

fn generate_system_html(system_info: &Value) -> String {
  format!(
    r#"
    <div class="system-info">
        <h2>⚙️ System Information</h2>
        <div class="system-grid">
            <div><strong>Runtime:</strong> {runtime}</div>
            <div><strong>Platform:</strong> {platform}</div>
            <div><strong>Memory Used:</strong> {memory_used}</div>
            <div><strong>Memory Available:</strong> {memory_available}</div>
            <div><strong>Render Time:</strong> {render_time}</div>
            <div><strong>Cache Hits:</strong> {cache_hits}</div>
            <div><strong>Requests Handled:</strong> {requests_handled}</div>
        </div>
        <div class="features">
            <h3>Features:</h3>
            <ul>{features_html}</ul>
        </div>
    </div>
    <style>
        .system-info {{ background: #f8f9fa; padding: 20px; border-radius: 8px; }}
        .system-grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin: 15px 0; }}
        .features ul {{ list-style-type: none; padding: 0; }}
        .features li {{ background: #007acc; color: white; padding: 5px 10px; margin: 5px 0; border-radius: 15px; display: inline-block; }}
    </style>
    "#,
    runtime = system_info["runtime"].as_str().unwrap_or("Unknown"),
    platform = system_info["platform"].as_str().unwrap_or("Unknown"),
    memory_used = system_info["memory"]["used"].as_str().unwrap_or("Unknown"),
    memory_available = system_info["memory"]["available"]
      .as_str()
      .unwrap_or("Unknown"),
    render_time = system_info["performance"]["renderTime"]
      .as_str()
      .unwrap_or("Unknown"),
    cache_hits = system_info["performance"]["cacheHits"]
      .as_i64()
      .unwrap_or(0),
    requests_handled = system_info["performance"]["requestsHandled"]
      .as_i64()
      .unwrap_or(0),
    features_html = system_info["features"]
      .as_array()
      .unwrap_or(&vec![])
      .iter()
      .map(|f| format!("<li>{}</li>", f.as_str().unwrap_or("")))
      .collect::<Vec<_>>()
      .join("")
  )
}

fn generate_business_html(business_data: &Value) -> String {
  let empty_vec = vec![];
  let sales_data = business_data["salesData"].as_array().unwrap_or(&empty_vec);
  let summary = &business_data["summary"];

  format!(
    r#"
    <div class="business-analytics">
        <h2>📊 Business Analytics - {period}</h2>
        <div class="summary-cards">
            <div class="card revenue">
                <h3>Total Revenue</h3>
                <div class="amount">${total_revenue}</div>
            </div>
            <div class="card expenses">
                <h3>Total Expenses</h3>
                <div class="amount">${total_expenses}</div>
            </div>
            <div class="card profit">
                <h3>Net Profit</h3>
                <div class="amount">${profit}</div>
                <div class="margin">Margin: {profit_margin}</div>
            </div>
        </div>
        <div class="monthly-data">
            <h3>Monthly Breakdown</h3>
            <table>
                <thead>
                    <tr><th>Month</th><th>Revenue</th><th>Expenses</th><th>Profit</th></tr>
                </thead>
                <tbody>
                    {monthly_rows}
                </tbody>
            </table>
        </div>
        <div class="calculated-at">Calculated at: {calculated_at}</div>
    </div>
    <style>
        .business-analytics {{ background: #fff; padding: 20px; border-radius: 8px; }}
        .summary-cards {{ display: flex; gap: 20px; margin: 20px 0; }}
        .card {{ flex: 1; padding: 15px; border-radius: 8px; text-align: center; }}
        .revenue {{ background: #d4edda; }}
        .expenses {{ background: #f8d7da; }}
        .profit {{ background: #d1ecf1; }}
        .amount {{ font-size: 2em; font-weight: bold; }}
        .margin {{ font-size: 0.9em; margin-top: 5px; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background: #f8f9fa; }}
    </style>
    "#,
    period = business_data["period"].as_str().unwrap_or("Unknown"),
    total_revenue = summary["totalRevenue"].as_i64().unwrap_or(0),
    total_expenses = summary["totalExpenses"].as_i64().unwrap_or(0),
    profit = summary["profit"].as_i64().unwrap_or(0),
    profit_margin = summary["profitMargin"].as_str().unwrap_or("0%"),
    monthly_rows = sales_data
      .iter()
      .map(|month| {
        let revenue = month["revenue"].as_i64().unwrap_or(0);
        let expenses = month["expenses"].as_i64().unwrap_or(0);
        let profit = revenue - expenses;
        format!(
          "<tr><td>{}</td><td>${}</td><td>${}</td><td>${}</td></tr>",
          month["month"].as_str().unwrap_or(""),
          revenue,
          expenses,
          profit
        )
      })
      .collect::<Vec<_>>()
      .join(""),
    calculated_at = business_data["calculatedAt"].as_str().unwrap_or("")
  )
}

// Widget generators for dashboard
fn generate_weather_widget(weather: &WeatherData) -> String {
  format!(
    "<div class='metric'><span>{}</span><strong>{}°F</strong></div><div \
     class='metric'><span>Condition</span><strong>{}</strong></div>",
    weather.city, weather.temperature, weather.conditions
  )
}

fn generate_profile_widget(profile: &UserProfile) -> String {
  format!(
    "<div class='metric'><span>User</span><strong>{} {}</strong></div><div \
     class='metric'><span>Projects</span><strong>{}</strong></div>",
    profile.profile.first_name, profile.profile.last_name, profile.stats.projects_created
  )
}

fn generate_system_widget(system_info: &Value) -> String {
  format!(
    "<div class='metric'><span>Runtime</span><strong>{}</strong></div><div \
     class='metric'><span>Memory</span><strong>{}</strong></div>",
    system_info["runtime"].as_str().unwrap_or("Unknown"),
    system_info["memory"]["used"].as_str().unwrap_or("Unknown")
  )
}

fn generate_business_widget(business_data: &Value) -> String {
  let summary = &business_data["summary"];
  format!(
    "<div class='metric'><span>Revenue</span><strong>${}</strong></div><div \
     class='metric'><span>Profit</span><strong>${}</strong></div>",
    summary["totalRevenue"].as_i64().unwrap_or(0),
    summary["profit"].as_i64().unwrap_or(0)
  )
}

// HTML rendering helpers
fn render_custom_html(content: &str, title: &str) -> Html<String> {
  let navigation = r#"
    <nav style="padding: 10px; background: #f0f0f0; margin-bottom: 20px; font-size: 14px;">
        <a href="/" style="margin-right: 8px;">Home</a> |
        <a href="/calc" style="margin: 0 8px;">Calculator</a> |
        <a href="/fetch" style="margin: 0 8px;">Fetch</a> |
        <a href="/data" style="margin: 0 8px;">Data</a> |
        <a href="/time" style="margin: 0 8px;">Time</a> |
        <a href="/weather" style="margin: 0 8px;">Weather</a> |
        <a href="/profile" style="margin: 0 8px;">Profile</a> |
        <a href="/system" style="margin: 0 8px;">System</a> |
        <a href="/business" style="margin: 0 8px;">Business</a> |
        <a href="/dashboard" style="margin: 0 8px;">Dashboard</a> |
        <a href="/v8" style="margin: 0 8px;">V8 Demo</a> |
        <a href="/v8/typescript" style="margin: 0 8px;">V8 TypeScript</a> |
        <a href="/v8/jsonplaceholder" style="margin: 0 8px;">JSONPlaceholder</a> |
        <a href="/stream-chat" style="margin-left: 8px;">Stream Chat</a>
    </nav>
    "#;

  let html = format!(
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
</head>
<body>
    {}
    <div style="margin: 20px;">
        {}
    </div>
    <footer style="margin-top: 40px; padding: 10px; background: #f9f9f9; text-align: center;">
        <small>Powered by ssr_rs • JS → Rust → HTML Pipeline Demo</small>
    </footer>
</body>
</html>"#,
    title, navigation, content
  );

  Html(html)
}

fn render_custom_html_with_css(content: &str, title: &str, css: &str) -> Html<String> {
  let navigation = r#"
    <nav style="padding: 10px; background: #f0f0f0; margin-bottom: 20px; font-size: 14px;">
        <a href="/" style="margin-right: 8px;">Home</a> |
        <a href="/calc" style="margin: 0 8px;">Calculator</a> |
        <a href="/fetch" style="margin: 0 8px;">Fetch</a> |
        <a href="/data" style="margin: 0 8px;">Data</a> |
        <a href="/time" style="margin: 0 8px;">Time</a> |
        <a href="/weather" style="margin: 0 8px;">Weather</a> |
        <a href="/profile" style="margin: 0 8px;">Profile</a> |
        <a href="/system" style="margin: 0 8px;">System</a> |
        <a href="/business" style="margin: 0 8px;">Business</a> |
        <a href="/dashboard" style="margin: 0 8px;">Dashboard</a> |
        <a href="/v8" style="margin: 0 8px;">V8 Demo</a> |
        <a href="/v8/typescript" style="margin: 0 8px;">V8 TypeScript</a> |
        <a href="/v8/jsonplaceholder" style="margin: 0 8px;">JSONPlaceholder</a> |
        <a href="/stream-chat" style="margin-left: 8px;">Stream Chat</a>
    </nav>
    "#;

  let html = format!(
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    {}
</head>
<body>
    {}
    {}
    <footer style="margin-top: 40px; padding: 10px; background: #f9f9f9; text-align: center;">
        <small>Powered by ssr_rs • JS → Rust → HTML Pipeline Demo</small>
    </footer>
</body>
</html>"#,
    title, css, navigation, content
  );

  Html(html)
}

// V8 Demo route (safe version that doesn't create isolates)
async fn v8_demo_safe() -> Html<String> {
  // Run the safe V8 simulation
  let start = Instant::now();
  let v8_results = vec![
    "V8 TypeScript integration successfully compiled and ready".to_string(),
    "TypeScript files: v8-processing.ts, data-generators.ts".to_string(),
    "Compiled JavaScript files available in client/dist/v8/".to_string(),
  ];
  let v8_info = format!(
    "V8 crate version: {} - Successfully integrated with TypeScript compilation pipeline!",
    env!("CARGO_PKG_VERSION")
  );
  let elapsed = start.elapsed();

  let v8_html = format!(
    r#"
    <div class="v8-demo">
      <h2>🚀 V8 JavaScript Engine Demo</h2>
      <div class="demo-info">
        <p>This demonstrates V8 crate integration with Rust (v8 crate successfully added!).</p>
        <p><strong>Processing Time:</strong> {:?}</p>
      </div>

      <div class="v8-info">
        <h3>V8 Integration Status:</h3>
        <pre>{}</pre>
      </div>

      <div class="v8-simulation">
        <h3>V8 Processing Simulation:</h3>
        <div class="simulation-results">
          {}
        </div>
      </div>

      <div class="integration-notes">
        <h3>Integration Notes:</h3>
        <ul>
          <li>✅ V8 crate successfully added to Cargo.toml</li>
          <li>✅ V8 processor module created</li>
          <li>✅ HTTP request processing structure implemented</li>
          <li>⚠️ In this setup, ssr_rs manages V8 runtime, so we simulate processing to avoid conflicts</li>
          <li>📚 For standalone V8 usage, initialize V8 separately from ssr_rs</li>
        </ul>
      </div>
    </div>
    <style>
      .v8-demo {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
      .demo-info, .v8-info, .v8-simulation, .integration-notes {{
        background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px;
      }}
      .simulation-results {{
        background: #f8f9fa; padding: 10px; border-radius: 5px; font-family: monospace; font-size: 0.9em;
      }}
      .simulation-results p {{ margin: 8px 0; padding: 5px; background: #e9ecef; border-radius: 3px; }}
      pre {{ background: #f8f9fa; padding: 10px; border-radius: 5px; overflow-x: auto; white-space: pre-wrap; }}
      ul {{ margin: 10px 0; }}
      li {{ margin: 5px 0; }}
    </style>
    "#,
    elapsed,
    v8_info,
    v8_results
      .iter()
      .map(|result| format!("<p>{}</p>", result))
      .collect::<Vec<_>>()
      .join("")
  );

  render_custom_html(&v8_html, "V8 JavaScript Engine Demo")
}

// V8 TypeScript Demo route - executes actual TypeScript logic with once_cell
async fn v8_typescript_demo() -> Html<String> {
  let start = Instant::now();

  // Get V8 code status using once_cell
  let v8_status = v8_processor::get_v8_code_status();

  // Process HTTP requests using the global V8 processor
  let http_processing_results = v8_processor::process_sample_requests();

  // Process data requests using the global V8 processor
  let data_processing_results = v8_processor::process_sample_data_requests();

  let elapsed = start.elapsed();

  let v8_html = format!(
    r#"
    <div class="v8-typescript-demo">
      <h2>🚀 V8 TypeScript Processing Demo</h2>

      <div class="demo-info">
        <p>This demonstrates real TypeScript processing using once_cell for global V8 storage.</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>TypeScript Files:</strong> client/src/v8-processing.ts, client/src/data-generators.ts</p>
      </div>

      <div class="v8-status">
        <h3>🔄 Once_Cell V8 Status</h3>
        <pre>{}</pre>
      </div>

      <div class="http-processing">
        <h3>HTTP Request Processing (TypeScript Logic via Once_Cell)</h3>
        <p>Using global V8 processor stored with once_cell:</p>
        <div class="results">
          {}
        </div>
      </div>

      <div class="data-processing">
        <h3>Data Generation Processing (TypeScript Logic via Once_Cell)</h3>
        <p>Using global V8 processor for data generation:</p>
        <div class="results">
          {}
        </div>
      </div>

      <div class="typescript-source">
        <h3>TypeScript Source Files</h3>
        <details>
          <summary>v8-processing.ts - HTTP Request Analysis</summary>
          <pre><code>interface HttpRequest {{
  path: string;
  referrer: string;
  host: string;
  user_agent: string;
}}

function processHttpRequest(request: HttpRequest): ProcessingResult {{
  // Analyzes request path, user agent, calculates risk score
  // Returns structured JSON with analysis results
}}</code></pre>
        </details>

        <details>
          <summary>data-generators.ts - Dynamic Data Generation</summary>
          <pre><code>function processDataRequest(requestType: string, params?: any): any {{
  // Generates user profiles, analytics data, etc.
  // Returns API-style responses with generated data
}}</code></pre>
        </details>
      </div>

      <div class="workflow">
        <h3>Once_Cell Processing Workflow</h3>
        <ol>
          <li>📝 TypeScript code written in client/src/</li>
          <li>🔧 TypeScript compiled to JavaScript in client/dist/v8/</li>
          <li>🔄 once_cell::Lazy loads and stores TypeScript code globally</li>
          <li>🚀 Global V8 processor executes TypeScript logic in Rust</li>
          <li>📊 JSON results generated and returned</li>
          <li>🎨 Rust renders JSON as formatted HTML</li>
        </ol>
      </div>

    </div>

    <style>
      .v8-typescript-demo {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
      .demo-info, .v8-status, .http-processing, .data-processing, .typescript-source, .workflow {{
        background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px;
      }}
      .results {{
        background: #f8f9fa; border-radius: 5px; padding: 10px; margin: 10px 0;
        max-height: 400px; overflow-y: auto;
      }}
      .results .json-result {{
        background: #e9ecef; padding: 8px; margin: 8px 0; border-radius: 3px;
        font-family: monospace; font-size: 0.85em; overflow-x: auto;
        white-space: pre-wrap; word-break: break-all;
      }}
      details {{ margin: 10px 0; }}
      summary {{ cursor: pointer; font-weight: bold; padding: 5px; background: #f0f0f0; border-radius: 3px; }}
      pre {{ background: #f8f9fa; padding: 10px; border-radius: 5px; overflow-x: auto; white-space: pre-wrap; }}
      code {{ font-family: 'Courier New', monospace; }}
      ol {{ padding-left: 20px; }}
      ol li {{ margin: 5px 0; }}
    </style>
    "#,
    elapsed,
    v8_status,
    http_processing_results
      .iter()
      .enumerate()
      .map(|(i, result)| format!(
        "<div class='json-result'><strong>HTTP Request {}:</strong><br/><pre>{}</pre></div>",
        i + 1,
        serde_json::from_str::<serde_json::Value>(result)
          .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| result.clone()))
          .unwrap_or_else(|_| result.clone())
      ))
      .collect::<Vec<_>>()
      .join(""),
    data_processing_results
      .iter()
      .enumerate()
      .map(|(i, result)| format!(
        "<div class='json-result'><strong>Data Request {}:</strong><br/><pre>{}</pre></div>",
        i + 1,
        serde_json::from_str::<serde_json::Value>(result)
          .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| result.clone()))
          .unwrap_or_else(|_| result.clone())
      ))
      .collect::<Vec<_>>()
      .join("")
  );

  render_custom_html(&v8_html, "V8 TypeScript Processing Demo")
}

// V8 JSONPlaceholder Demo route - demonstrates simulated API data processing
async fn v8_jsonplaceholder_demo() -> Html<String> {
  let start = Instant::now();

  // Get V8 code status using once_cell
  let v8_status = v8_processor::get_v8_code_status();

  // Process JSONPlaceholder requests using the global V8 processor
  let jsonplaceholder_results = v8_processor::process_jsonplaceholder_samples();

  let elapsed = start.elapsed();

  let v8_html = format!(
    r#"
    <div class="v8-jsonplaceholder-demo">
      <h2>🌐 V8 JSONPlaceholder API Demo</h2>

      <div class="demo-info">
        <p>This demonstrates V8 processing of JSONPlaceholder-style API data using TypeScript logic.</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>API Source:</strong> <a href="https://jsonplaceholder.typicode.com/" target="_blank">JSONPlaceholder</a> (simulated)</p>
        <p><strong>TypeScript File:</strong> client/src/jsonplaceholder-demo.ts</p>
      </div>

      <div class="v8-status">
        <h3>🔄 V8 Status</h3>
        <pre>{}</pre>
      </div>

      <div class="api-examples">
        <h3>📊 JSONPlaceholder API Examples</h3>
        <p>The following examples demonstrate various JSONPlaceholder API endpoints processed by V8:</p>

        <div class="api-grid">
          <div class="api-card">
            <h4>📝 Posts</h4>
            <p>Blog posts with titles and content</p>
            <code>GET /posts</code>, <code>GET /posts/1</code>
          </div>

          <div class="api-card">
            <h4>👥 Users</h4>
            <p>User profiles with contact information</p>
            <code>GET /users</code>, <code>GET /users/2</code>
          </div>

          <div class="api-card">
            <h4>✅ Todos</h4>
            <p>Task management with completion status</p>
            <code>GET /todos</code>
          </div>

          <div class="api-card">
            <h4>🔗 User Posts</h4>
            <p>Aggregated user data with their posts</p>
            <code>GET /users/1/posts</code>
          </div>

          <div class="api-card">
            <h4>📈 Analytics</h4>
            <p>Statistical analysis of all data</p>
            <code>GET /analytics</code>
          </div>
        </div>
      </div>

      <div class="jsonplaceholder-processing">
        <h3>API Processing Results (V8 + TypeScript)</h3>
        <p>Each result shows the TypeScript function processing JSONPlaceholder-style data:</p>
        <div class="results">
          {}
        </div>
      </div>

      <div class="typescript-source">
        <h3>TypeScript Implementation</h3>
        <details>
          <summary>jsonplaceholder-demo.ts - API Processing Functions</summary>
          <pre><code>// Main function to simulate JSONPlaceholder API calls
function fetchJsonPlaceholderData(endpoint: string, id?: number): any {{
  // Simulates: /posts, /users, /comments, /albums, /photos, /todos
  // Returns structured API responses with metadata
}}

// Helper function to get user's posts
function getUserPosts(userId: number): any {{
  // Aggregates user data with their posts
  // Calculates statistics like post count and average lengths
}}

// Analytics function for JSONPlaceholder data
function analyzeJsonPlaceholderData(): any {{
  // Performs comprehensive analysis of all data types
  // Returns statistics, distributions, and insights
}}</code></pre>
        </details>
      </div>

      <div class="workflow">
        <h3>Processing Workflow</h3>
        <ol>
          <li>📝 TypeScript defines JSONPlaceholder API interfaces and sample data</li>
          <li>🔧 TypeScript compiled to JavaScript with full type checking</li>
          <li>🔄 once_cell loads the compiled JSONPlaceholder processing code</li>
          <li>🌐 V8 processor simulates various API endpoints (/posts, /users, /todos)</li>
          <li>📊 TypeScript logic processes requests with metadata and caching info</li>
          <li>📈 Advanced features: user aggregation, analytics, error handling</li>
          <li>🎨 Rust renders all results as formatted HTML with JSON previews</li>
        </ol>
      </div>

      <div class="api-features">
        <h3>🚀 Advanced Features</h3>
        <ul>
          <li>✅ <strong>Full CRUD Simulation:</strong> GET requests for posts, users, comments, albums, photos, todos</li>
          <li>✅ <strong>Error Handling:</strong> 404 responses for non-existent resources</li>
          <li>✅ <strong>Metadata Support:</strong> Response times, caching indicators, API source attribution</li>
          <li>✅ <strong>Data Relationships:</strong> User posts aggregation, cross-referencing</li>
          <li>✅ <strong>Analytics Engine:</strong> Comprehensive statistics and data analysis</li>
          <li>✅ <strong>TypeScript Types:</strong> Full interface definitions for all data structures</li>
          <li>✅ <strong>Response Consistency:</strong> Matches real JSONPlaceholder API structure</li>
        </ul>
      </div>

    </div>

    <style>
      .v8-jsonplaceholder-demo {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
      .demo-info, .v8-status, .api-examples, .jsonplaceholder-processing, .typescript-source, .workflow, .api-features {{
        background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px;
      }}

      .api-grid {{
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 15px;
        margin: 15px 0;
      }}

      .api-card {{
        border: 1px solid #e0e0e0;
        border-radius: 8px;
        padding: 15px;
        background: #f8f9fa;
        text-align: center;
      }}

      .api-card h4 {{
        color: #007acc;
        margin: 0 0 10px 0;
      }}

      .api-card code {{
        background: #e9ecef;
        padding: 2px 6px;
        border-radius: 3px;
        font-size: 0.85em;
        display: block;
        margin: 5px 0;
      }}

      .results {{
        background: #f8f9fa;
        border-radius: 5px;
        padding: 10px;
        margin: 10px 0;
        max-height: 500px;
        overflow-y: auto;
      }}

      .results .json-result {{
        background: #e9ecef;
        padding: 12px;
        margin: 12px 0;
        border-radius: 5px;
        font-family: monospace;
        font-size: 0.85em;
        overflow-x: auto;
        white-space: pre-wrap;
        word-break: break-all;
        border-left: 4px solid #007acc;
      }}

      details {{ margin: 10px 0; }}
      summary {{
        cursor: pointer;
        font-weight: bold;
        padding: 8px;
        background: #f0f0f0;
        border-radius: 5px;
        border: 1px solid #ddd;
      }}
      summary:hover {{ background: #e8e8e8; }}

      pre {{
        background: #f8f9fa;
        padding: 15px;
        border-radius: 5px;
        overflow-x: auto;
        white-space: pre-wrap;
        border: 1px solid #e9ecef;
      }}

      code {{ font-family: 'Consolas', 'Monaco', 'Courier New', monospace; }}

      ol {{ padding-left: 20px; }}
      ol li {{ margin: 8px 0; padding: 2px 0; }}

      ul {{ padding-left: 20px; }}
      ul li {{ margin: 5px 0; }}

      .demo-info a {{ color: #007acc; text-decoration: none; }}
      .demo-info a:hover {{ text-decoration: underline; }}
    </style>
    "#,
    elapsed,
    v8_status,
    jsonplaceholder_results
      .iter()
      .enumerate()
      .map(|(i, result)| {
        let titles = vec![
          "All Posts",
          "Single Post (ID: 1)",
          "Single User (ID: 2)",
          "All Todos",
          "User Posts (User ID: 1)",
          "Data Analytics Overview",
        ];
        let title = titles.get(i).unwrap_or(&"API Result");

        format!(
          "<div class='json-result'><strong>{}:</strong><br/><pre>{}</pre></div>",
          title,
          serde_json::from_str::<serde_json::Value>(result)
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| result.clone()))
            .unwrap_or_else(|_| result.clone())
        )
      })
      .collect::<Vec<_>>()
      .join("")
  );

  render_custom_html(&v8_html, "V8 JSONPlaceholder API Demo")
}

// Stream Chat Demo using V8 TypeScript processor
async fn stream_chat_demo(Query(params): Query<QueryParams>) -> Html<String> {
  let start = Instant::now();

  // Get V8 processor status
  let v8_status = v8_processor::get_v8_code_status();

  // Create a V8 TypeScript processor
  let mut processor = match v8_processor::V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => {
      let error_html = format!(
        r#"
        <div class="error-container">
          <h2>❌ V8 TypeScript Processor Not Available</h2>
          <p>The V8 TypeScript code files could not be loaded.</p>
          <p>Status: {}</p>
          <p>Please ensure the client/dist/v8/ files exist and are properly compiled.</p>
        </div>
        "#,
        v8_status
      );
      return render_custom_html(&error_html, "Stream Chat Demo - Error");
    }
  };

  // Determine which demo to run based on query parameter
  let demo_type = params.demo.as_deref().unwrap_or("setup");

  let (demo_title, demo_results) = match demo_type {
    "authenticate" => {
      let user_id = params.data.as_deref().unwrap_or("john");
      let auth_result = Some(simple_v8_executor::SimpleV8Executor::execute_stream_chat(
        "authenticate",
        Some(user_id),
      ));
      (
        format!("Stream Chat Authentication - User: {}", user_id),
        vec![auth_result.unwrap_or_else(|| "Failed to authenticate user".to_string())],
      )
    }
    "user-context" => {
      let user_id = params.data.as_deref().unwrap_or("john");
      let context_result = Some(simple_v8_executor::SimpleV8Executor::execute_stream_chat(
        "user-context",
        Some(user_id),
      ));
      (
        format!("Stream Chat User Context - User: {}", user_id),
        vec![context_result.unwrap_or_else(|| "Failed to get user context".to_string())],
      )
    }
    "analytics" => {
      let analytics_result = Some(simple_v8_executor::SimpleV8Executor::execute_stream_chat(
        "analytics",
        None,
      ));
      (
        "Stream Chat Analytics".to_string(),
        vec![analytics_result.unwrap_or_else(|| "Failed to get analytics".to_string())],
      )
    }
    _ => {
      // Default to setup demo
      let setup_result = Some(simple_v8_executor::SimpleV8Executor::execute_stream_chat(
        "setup", None,
      ));
      (
        "Stream Chat Demo Setup & Configuration".to_string(),
        vec![setup_result.unwrap_or_else(|| "Failed to get setup info".to_string())],
      )
    }
  };

  let stream_chat_html = format!(
    r#"
    <div class="demo-container">
      <h1>🚀 Stream Chat Demo</h1>
      <div class="demo-info">
        <p><strong>Demo:</strong> {}</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>V8 Processor Status:</strong> Available ✅</p>
        <p><strong>Stream Chat API Pattern:</strong> Following <a href="https://getstream.io/chat/docs/react/tokens_and_authentication/" target="_blank">official documentation</a></p>
      </div>

      <div class="controls">
        <h3>Available Demos:</h3>
        <ul>
          <li><a href="/stream-chat?demo=setup">Setup & Configuration</a> - API keys, users, channels</li>
          <li><a href="/stream-chat?demo=authenticate&data=john">Authenticate User (john)</a></li>
          <li><a href="/stream-chat?demo=authenticate&data=jane">Authenticate User (jane)</a></li>
          <li><a href="/stream-chat?demo=authenticate&data=bob">Authenticate User (bob)</a></li>
          <li><a href="/stream-chat?demo=user-context&data=john">User Context (john)</a></li>
          <li><a href="/stream-chat?demo=user-context&data=jane">User Context (jane)</a></li>
          <li><a href="/stream-chat?demo=analytics">Analytics Overview</a></li>
        </ul>
      </div>

      <div class="stream-chat-integration">
        <h3>🔧 Stream Chat Integration Pattern</h3>
        <div class="code-example">
          <h4>Server-side Token Generation:</h4>
          <pre><code>// Initialize Stream Chat Server Client
const api_key = "your_api_key";
const api_secret = "your_api_secret";
const serverClient = StreamChat.getInstance(api_key, api_secret);

// Create user token
const user_id = "john";
const token = serverClient.createToken(user_id);

// With expiration (optional)
const expireTime = Math.floor(Date.now() / 1000) + 60 * 60; // 1 hour
const tokenWithExp = serverClient.createToken(user_id, expireTime);

// With issued at time (security best practice)
const issuedAt = Math.floor(Date.now() / 1000);
const secureToken = serverClient.createToken(user_id, expireTime, issuedAt);</code></pre>

          <h4>Client-side Connection:</h4>
          <pre><code>// Connect user with token from server
await client.connectUser({{
  id: "john",
  name: "John Doe",
  image: "https://avatar.example.com/john.jpg"
}}, tokenFromServer);

// Or use token provider for automatic refresh
await client.connectUser(userObject, async () => {{
  const response = await fetch('/api/chat-token', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{ user_id: "john" }})
  }});
  const data = await response.json();
  return data.token;
}});</code></pre>
        </div>
      </div>

      <div class="results">
        <h3>📊 Demo Results:</h3>
        {}
      </div>

      <div class="v8-status">
        <details>
          <summary>🔧 V8 TypeScript Processor Status</summary>
          <pre>{}</pre>
        </details>
      </div>
    </div>

    <style>
      .demo-container {{
        max-width: 1200px;
        margin: 0 auto;
        padding: 20px;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
      }}

      .demo-info {{
        background: #f8f9fa;
        padding: 15px;
        border-radius: 8px;
        margin: 20px 0;
        border-left: 4px solid #28a745;
      }}

      .controls {{
        background: #e3f2fd;
        padding: 15px;
        border-radius: 8px;
        margin: 20px 0;
      }}

      .controls ul {{ margin: 10px 0; }}
      .controls li {{ margin: 5px 0; }}
      .controls a {{ color: #1976d2; text-decoration: none; }}
      .controls a:hover {{ text-decoration: underline; }}

      .stream-chat-integration {{
        background: #fff3e0;
        padding: 20px;
        border-radius: 8px;
        margin: 20px 0;
        border-left: 4px solid #ff9800;
      }}

      .code-example h4 {{
        color: #e65100;
        margin: 15px 0 8px 0;
      }}

      .results {{
        background: #f5f5f5;
        padding: 15px;
        border-radius: 8px;
        margin: 20px 0;
        max-height: 600px;
        overflow-y: auto;
      }}

      .results .json-result {{
        background: #e8f5e8;
        padding: 15px;
        margin: 15px 0;
        border-radius: 8px;
        font-family: 'Consolas', 'Monaco', monospace;
        font-size: 0.9em;
        overflow-x: auto;
        white-space: pre-wrap;
        word-break: break-all;
        border-left: 4px solid #4caf50;
      }}

      .v8-status {{
        margin: 20px 0;
      }}

      details {{ margin: 10px 0; }}
      summary {{
        cursor: pointer;
        font-weight: bold;
        padding: 10px;
        background: #f0f0f0;
        border-radius: 6px;
        border: 1px solid #ddd;
      }}
      summary:hover {{ background: #e8e8e8; }}

      pre {{
        background: #f8f9fa;
        padding: 15px;
        border-radius: 6px;
        overflow-x: auto;
        white-space: pre-wrap;
        border: 1px solid #e9ecef;
        margin: 10px 0;
      }}

      code {{
        font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
        background: #f1f3f4;
        padding: 2px 4px;
        border-radius: 3px;
        font-size: 0.9em;
      }}

      .error-container {{
        background: #ffebee;
        padding: 20px;
        border-radius: 8px;
        border-left: 4px solid #f44336;
        color: #c62828;
      }}

      h1 {{ color: #1976d2; }}
      h3 {{ color: #424242; margin: 20px 0 10px 0; }}
    </style>
    "#,
    demo_title,
    start.elapsed(),
    demo_results
      .iter()
      .map(|result| {
        format!(
          "<div class='json-result'><pre>{}</pre></div>",
          serde_json::from_str::<serde_json::Value>(result)
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| result.clone()))
            .unwrap_or_else(|_| result.clone())
        )
      })
      .collect::<Vec<_>>()
      .join(""),
    v8_status
  );

  render_custom_html(&stream_chat_html, "Stream Chat Demo")
}

// Stream Chat Authentication endpoint - prints JSON result
async fn stream_chat_authenticate(Query(params): Query<QueryParams>) -> Html<String> {
  let start = Instant::now();

  let user_id = params.data.as_deref().unwrap_or("john");
  let auth_result =
    real_v8_executor::RealV8Executor::execute_stream_chat_js("authenticate", Some(user_id));

  match auth_result {
    Ok(html_result) => {
      println!(
        "Stream Chat Authenticate Success for {}: HTML rendered via V8",
        user_id
      );
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>Stream Chat V8 Authentication - User: {}</h2>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>Execution Method:</strong> V8 JavaScript Engine with HTML Rendering</p>
        <div style="margin: 20px 0;">
          {}
        </div>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        user_id,
        start.elapsed(),
        html_result
      ))
    }
    Err(error) => {
      println!("Stream Chat Authenticate Error for {}: {}", user_id, error);
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>❌ Stream Chat V8 Error</h2>
        <p><strong>User:</strong> {}</p>
        <p><strong>Error:</strong> {}</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        user_id,
        error,
        start.elapsed()
      ))
    }
  }
}

// Stream Chat User Context endpoint - prints JSON result
async fn stream_chat_user_context(Query(params): Query<QueryParams>) -> Html<String> {
  let start = Instant::now();

  let user_id = params.data.as_deref().unwrap_or("john");
  let context_result =
    real_v8_executor::RealV8Executor::execute_stream_chat_js("user-context", Some(user_id));

  match context_result {
    Ok(html_result) => {
      println!(
        "Stream Chat User Context Success for {}: HTML rendered via V8",
        user_id
      );
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>Stream Chat V8 User Context - User: {}</h2>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>Execution Method:</strong> V8 JavaScript Engine with HTML Rendering</p>
        <div style="margin: 20px 0;">
          {}
        </div>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        user_id,
        start.elapsed(),
        html_result
      ))
    }
    Err(error) => {
      println!("Stream Chat User Context Error for {}: {}", user_id, error);
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>❌ Stream Chat V8 Error</h2>
        <p><strong>User:</strong> {}</p>
        <p><strong>Error:</strong> {}</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        user_id,
        error,
        start.elapsed()
      ))
    }
  }
}

// Stream Chat Analytics endpoint - prints JSON result
async fn stream_chat_analytics() -> Html<String> {
  let start = Instant::now();

  let analytics_result =
    real_v8_executor::RealV8Executor::execute_stream_chat_js("analytics", None);

  match analytics_result {
    Ok(html_result) => {
      println!("Stream Chat Analytics Success: HTML rendered via V8");
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>Stream Chat V8 Analytics</h2>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>Execution Method:</strong> V8 JavaScript Engine with HTML Rendering</p>
        <div style="margin: 20px 0;">
          {}
        </div>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        start.elapsed(),
        html_result
      ))
    }
    Err(error) => {
      println!("Stream Chat Analytics Error: {}", error);
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>❌ Stream Chat V8 Error</h2>
        <p><strong>Error:</strong> {}</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        error,
        start.elapsed()
      ))
    }
  }
}

// Stream Chat Setup endpoint - prints JSON result
async fn stream_chat_setup() -> Html<String> {
  let start = Instant::now();

  let setup_result = real_v8_executor::RealV8Executor::execute_stream_chat_js("setup", None);

  match setup_result {
    Ok(html_result) => {
      println!("Stream Chat Setup Success: HTML rendered via V8");
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>Stream Chat V8 Setup & Configuration</h2>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><strong>Execution Method:</strong> V8 JavaScript Engine with HTML Rendering</p>
        <div style="margin: 20px 0;">
          {}
        </div>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        start.elapsed(),
        html_result
      ))
    }
    Err(error) => {
      println!("Stream Chat Setup Error: {}", error);
      Html(format!(
        r#"<div style="font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px;">
        <h2>❌ Stream Chat V8 Error</h2>
        <p><strong>Error:</strong> {}</p>
        <p><strong>Processing Time:</strong> {:?}</p>
        <p><a href="/stream-chat" style="color: #007acc; text-decoration: none;">← Back to Stream Chat Demo</a></p>
        </div>"#,
        error,
        start.elapsed()
      ))
    }
  }
}

// Stream Chat Token Generation Demo - shows the complete authentication flow
async fn stream_chat_token_demo(Query(params): Query<QueryParams>) -> Html<String> {
  use crate::config::STREAM_CONFIG;

  let start = Instant::now();
  let user_id = params.data.as_deref().unwrap_or("john");

  // Get authentication result using simple V8 executor
  let auth_result = Some(simple_v8_executor::SimpleV8Executor::execute_stream_chat(
    "authenticate",
    Some(user_id),
  ));

  let (token, token_details) = match auth_result {
    Some(result) => match serde_json::from_str::<serde_json::Value>(&result) {
      Ok(json) => {
        let token = json
          .get("token")
          .and_then(|t| t.as_str())
          .unwrap_or("N/A")
          .to_string();
        (token, serde_json::to_string_pretty(&json).unwrap_or(result))
      }
      Err(_) => ("Error".to_string(), result),
    },
    None => (
      "Failed to generate".to_string(),
      "No response from processor".to_string(),
    ),
  };

  let demo_html = format!(
    r#"
    <div class="token-demo">
      <h1>🔐 Stream Chat Token Generation Demo</h1>

      <div class="code-section">
        <h2>The Code Pattern (from Stream.io docs)</h2>
        <pre class="code-block"><code>// Define values
const api_key = "{}";
const api_secret = "{}";
const user_id = "{}";

// Initialize a Server Client
const serverClient = StreamChat.getInstance(api_key, api_secret);

// Create User Token
const token = serverClient.createToken(user_id);</code></pre>
      </div>

      <div class="implementation-section">
        <h2>Our Rust Implementation</h2>
        <pre class="code-block"><code>// Load credentials from environment
let api_key = env::var("STREAM_API_KEY").unwrap_or("demo_key");
let api_secret = env::var("STREAM_API_SECRET").unwrap_or("demo_secret");

// Process through V8 TypeScript
let processor = V8TypeScriptProcessor::new();
let token = processor.authenticate_stream_user("{}", None, None);</code></pre>
      </div>

      <div class="result-section">
        <h2>Generated Token Result</h2>
        <div class="token-display">
          <strong>Token:</strong> <code class="token">{}</code>
        </div>

        <h3>Full Response:</h3>
        <pre class="result-json">{}</pre>
      </div>

      <div class="config-info">
        <h2>Current Configuration</h2>
        <ul>
          <li><strong>API Key:</strong> <code>{}</code></li>
          <li><strong>API Secret:</strong> <code>{}***</code> (hidden for security)</li>
          <li><strong>User ID:</strong> <code>{}</code></li>
          <li><strong>Processing Time:</strong> {:?}</li>
        </ul>
      </div>

      <div class="try-section">
        <h2>Try Different Users</h2>
        <div class="user-links">
          <a href="/stream-chat/token?data=john" class="user-link">Generate for John</a>
          <a href="/stream-chat/token?data=jane" class="user-link">Generate for Jane</a>
          <a href="/stream-chat/token?data=alice" class="user-link">Generate for Alice</a>
          <a href="/stream-chat/token?data=bob" class="user-link">Generate for Bob</a>
        </div>
      </div>

      <div class="navigation">
        <a href="/stream-chat" class="nav-link">← Back to Stream Chat Demo</a>
      </div>
    </div>

    <style>
      .token-demo {{
        max-width: 1000px;
        margin: 0 auto;
        padding: 20px;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      }}

      h1 {{
        color: #005fff;
        margin-bottom: 30px;
      }}

      .code-section, .implementation-section, .result-section, .config-info, .try-section {{
        background: #f5f5f5;
        padding: 20px;
        border-radius: 8px;
        margin-bottom: 20px;
      }}

      .code-block {{
        background: #282c34;
        color: #abb2bf;
        padding: 15px;
        border-radius: 5px;
        overflow-x: auto;
      }}

      .code-block code {{
        font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
        font-size: 14px;
        line-height: 1.5;
      }}

      .token-display {{
        background: white;
        padding: 15px;
        border-radius: 5px;
        margin-bottom: 15px;
        border: 1px solid #ddd;
      }}

      .token {{
        background: #e3f2fd;
        padding: 2px 6px;
        border-radius: 3px;
        font-family: monospace;
        word-break: break-all;
      }}

      .result-json {{
        background: white;
        padding: 15px;
        border-radius: 5px;
        border: 1px solid #ddd;
        overflow-x: auto;
      }}

      .config-info ul {{
        list-style: none;
        padding: 0;
      }}

      .config-info li {{
        padding: 8px 0;
        border-bottom: 1px solid #eee;
      }}

      .config-info li:last-child {{
        border-bottom: none;
      }}

      .user-links {{
        display: flex;
        gap: 10px;
        flex-wrap: wrap;
      }}

      .user-link {{
        background: #005fff;
        color: white;
        padding: 10px 20px;
        text-decoration: none;
        border-radius: 5px;
        transition: background 0.3s;
      }}

      .user-link:hover {{
        background: #0047d0;
      }}

      .nav-link {{
        display: inline-block;
        margin-top: 20px;
        color: #005fff;
        text-decoration: none;
      }}

      .nav-link:hover {{
        text-decoration: underline;
      }}

      .error {{
        background: #fee;
        color: #c00;
        padding: 20px;
        border-radius: 5px;
      }}
    </style>
    "#,
    STREAM_CONFIG.api_key,
    if STREAM_CONFIG.api_secret.len() > 8 {
      &STREAM_CONFIG.api_secret[.. 8]
    } else {
      &STREAM_CONFIG.api_secret
    },
    user_id,
    user_id,
    if token.len() > 50 {
      format!("{}...", &token[.. 50])
    } else {
      token.clone()
    },
    token_details,
    STREAM_CONFIG.api_key,
    if STREAM_CONFIG.api_secret.len() > 8 {
      &STREAM_CONFIG.api_secret[.. 8]
    } else {
      &STREAM_CONFIG.api_secret
    },
    user_id,
    start.elapsed()
  );

  Html(demo_html)
}
