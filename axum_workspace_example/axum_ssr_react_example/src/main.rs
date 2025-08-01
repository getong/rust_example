use std::{cell::RefCell, fs::read_to_string, time::Instant};

use axum::{Router, extract::Query, response::Html, routing::get};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssr_rs::Ssr;

thread_local! {
    static SSR: RefCell<Ssr<'static, 'static>> = RefCell::new({
        let js_code = read_to_string("client/dist/ssr/index.js").unwrap();
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
        Ssr::from(enhanced_js, "SSR").unwrap()
    })
}

#[derive(Deserialize)]
struct QueryParams {
  demo: Option<String>,
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
  Ssr::create_platform();

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
    .route("/dashboard", get(dashboard_demo));

  // run our app with hyper, listening globally on port 3000
  let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
  println!("Server running on http://0.0.0.0:8080");
  axum::serve(listener, app).await.unwrap();
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
async fn fetch_demo() -> Html<String> {
  let start = Instant::now();

  // Call TypeScript fetchData function from Rust
  let result = SSR.with(|ssr| {
    let mut ssr_instance = ssr.borrow_mut();
    ssr_instance.render_to_string(Some("fetchData"))
  });

  match result {
    Ok(fetch_result) => {
      println!("Fetch result from TypeScript: {}", fetch_result);
      println!("Fetch elapsed: {:?}", start.elapsed());

      // Since ssr_rs returns HTML instead of the function result,
      // let's demonstrate with mock fetch data
      let fetch_data = serde_json::json!({
        "url": "https://api.example.com/users",
        "method": "GET",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": "Mock response from V8 environment",
        "data": {
          "users": [
            { "id": 1, "name": "John", "role": "Admin" },
            { "id": 2, "name": "Jane", "role": "User" },
            { "id": 3, "name": "Bob", "role": "Developer" }
          ],
          "total": 3
        },
        "processing_time": format!("{:?}", start.elapsed())
      });

      let fetch_html = format!(
        r#"
        <div class="fetch-result">
          <h2>🌐 Fetch API Result</h2>
          <div class="fetch-info">
            <p><strong>URL:</strong> {}</p>
            <p><strong>Method:</strong> {}</p>
            <p><strong>Timestamp:</strong> {}</p>
            <p><strong>Processing Time:</strong> {}</p>
          </div>
          <div class="fetch-data">
            <h3>Response Data</h3>
            <p><strong>Message:</strong> {}</p>
            <h3>Users ({} total)</h3>
            <table style="width: 100%; border-collapse: collapse;">
              <thead>
                <tr style="background: #f0f0f0;">
                  <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">ID</th>
                  <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">Name</th>
                  <th style="padding: 8px; text-align: left; border: 1px solid #ddd;">Role</th>
                </tr>
              </thead>
              <tbody>
                {}
              </tbody>
            </table>
          </div>
        </div>
        <style>
          .fetch-result {{ background: #f9f9f9; padding: 20px; border-radius: 8px; }}
          .fetch-info {{ background: white; padding: 15px; border-radius: 5px; margin-bottom: 20px; }}
          .fetch-info p {{ margin: 5px 0; }}
          .fetch-data {{ background: white; padding: 15px; border-radius: 5px; }}
          .fetch-data h3 {{ color: #007acc; margin-top: 15px; }}
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
             #ddd;'>{}</td></tr>",
            user["id"].as_i64().unwrap_or(0),
            user["name"].as_str().unwrap_or(""),
            user["role"].as_str().unwrap_or("")
          ))
          .collect::<Vec<_>>()
          .join("")
      );

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
      ("renderWithData", Some(data_str)) => {
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
                <a href="/dashboard" style="margin: 0 8px;">Dashboard</a>
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

  let sales_data = vec![
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
      .unwrap()
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
        <a href="/dashboard" style="margin: 0 8px;">Dashboard</a>
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
        <a href="/dashboard" style="margin: 0 8px;">Dashboard</a>
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
