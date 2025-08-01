use std::{fs, sync::Mutex};

use once_cell::sync::Lazy;
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

// Global V8 TypeScript code storage
#[derive(Debug, Clone)]
pub struct V8TypeScriptCode {
  pub v8_processing_js: String,
  pub data_generators_js: String,
}

impl V8TypeScriptCode {
  pub fn new() -> Option<Self> {
    // Load the compiled TypeScript files
    match (
      fs::read_to_string("client/dist/v8/v8-processing.js"),
      fs::read_to_string("client/dist/v8/data-generators.js"),
    ) {
      (Ok(v8_processing_js), Ok(data_generators_js)) => Some(Self {
        v8_processing_js,
        data_generators_js,
      }),
      _ => None,
    }
  }
}

// Global V8 code instance using once_cell
static V8_CODE: Lazy<Mutex<Option<V8TypeScriptCode>>> =
  Lazy::new(|| Mutex::new(V8TypeScriptCode::new()));

// Since we're using ssr_rs which manages V8, we'll create a simpler processor
// that works with the existing V8 runtime
pub struct V8TypeScriptProcessor;

impl V8TypeScriptProcessor {
  pub fn new() -> Option<Self> {
    // Check if V8 code is loaded
    match V8_CODE.lock() {
      Ok(guard) => {
        if guard.is_some() {
          Some(Self)
        } else {
          None
        }
      }
      Err(_) => None,
    }
  }

  // Simulate processing using the loaded TypeScript code
  pub fn process_http_request(&self, request: &dyn HttpRequest) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    // Since we can't easily create new V8 isolates with ssr_rs managing V8,
    // we'll simulate the processing based on the TypeScript logic
    let path = request.path();
    let host = request.host();
    let user_agent = request.user_agent();

    // Simulate the TypeScript processHttpRequest function logic
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let is_api = path.starts_with("/api");
    let is_static_asset = path.ends_with(".js")
      || path.ends_with(".css")
      || path.ends_with(".png")
      || path.ends_with(".jpg")
      || path.ends_with(".gif")
      || path.ends_with(".svg");

    // Simple user agent analysis
    let user_agent_lower = user_agent.to_lowercase();
    let browser = if user_agent_lower.contains("chrome") {
      "chrome"
    } else if user_agent_lower.contains("firefox") {
      "firefox"
    } else if user_agent_lower.contains("safari") {
      "safari"
    } else if user_agent_lower.contains("curl") || user_agent_lower.contains("bot") {
      "bot"
    } else {
      "unknown"
    };

    let is_bot = browser == "bot";

    // Calculate risk score
    let mut risk_score = 0;
    if is_bot {
      risk_score += 30;
    }
    if path.contains("admin") {
      risk_score += 40;
    }
    if path.contains("sql") || path.contains("script") {
      risk_score += 50;
    }

    // Generate response
    let status = if risk_score > 70 {
      "blocked"
    } else {
      "processed"
    };
    let message = if risk_score > 70 {
      format!("High risk request blocked: {}", path)
    } else {
      format!("Successfully processed {}", path)
    };

    let should_cache = is_static_asset && !is_api;
    let redirect_url = if path == "/old-page" {
      Some("/new-page")
    } else {
      None
    };

    // Create JSON response matching TypeScript interface
    let result = serde_json::json!({
      "status": status,
      "timestamp": chrono::Utc::now().to_rfc3339(),
      "request": {
        "path": path,
        "referrer": request.referrer(),
        "host": host,
        "user_agent": user_agent
      },
      "analysis": {
        "path_info": {
          "is_api": is_api,
          "is_static_asset": is_static_asset,
          "segments": path_segments
        },
        "user_agent_info": {
          "browser": browser,
          "is_bot": is_bot
        },
        "risk_score": risk_score
      },
      "response": {
        "message": message,
        "should_cache": should_cache,
        "redirect_url": redirect_url
      }
    });

    Some(result.to_string())
  }

  pub fn process_data_request(&self, request_type: &str, params: Option<&str>) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    // Simulate the TypeScript processDataRequest function logic
    let result = match request_type {
      "user" => {
        let user_id = params
          .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
          .and_then(|v| v.get("id").and_then(|id| id.as_i64()))
          .unwrap_or(1);

        let first_names = [
          "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry",
        ];
        let last_names = [
          "Smith", "Johnson", "Brown", "Davis", "Wilson", "Miller", "Taylor", "Anderson",
        ];

        let first_name = first_names[(user_id as usize) % first_names.len()];
        let last_name = last_names[((user_id * 3) as usize) % last_names.len()];
        let username = format!(
          "{}{}{}",
          first_name.to_lowercase(),
          last_name.to_lowercase(),
          user_id
        );

        serde_json::json!({
          "success": true,
          "data": {
            "id": user_id,
            "username": username,
            "email": format!("{}@example.com", username),
            "created_at": chrono::Utc::now().to_rfc3339(),
            "profile": {
              "first_name": first_name,
              "last_name": last_name,
              "bio": format!("Hi, I'm {}! Welcome to my profile.", first_name),
              "avatar_url": format!("https://avatar.example.com/{}.jpg", username)
            },
            "settings": {
              "theme": if user_id % 2 == 0 { "dark" } else { "light" },
              "notifications": user_id % 3 != 0,
              "language": if user_id % 5 == 0 { "es" } else { "en" }
            },
            "stats": {
              "posts_count": (user_id * 7) % 100,
              "followers_count": (user_id * 23) % 1000,
              "following_count": (user_id * 13) % 500
            }
          },
          "timestamp": chrono::Utc::now().to_rfc3339(),
          "processing_time_ms": 45
        })
      }

      "users" => {
        let count = params
          .and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok())
          .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
          .unwrap_or(5) as usize;

        let users: Vec<serde_json::Value> = (1 ..= count)
          .map(|i| {
            let first_names = ["Alice", "Bob", "Charlie", "Diana", "Eve"];
            let last_names = ["Smith", "Johnson", "Brown", "Davis", "Wilson"];

            let first_name = first_names[i % first_names.len()];
            let last_name = last_names[(i * 2) % last_names.len()];
            let username = format!(
              "{}{}{}",
              first_name.to_lowercase(),
              last_name.to_lowercase(),
              i
            );

            serde_json::json!({
              "id": i,
              "username": username,
              "email": format!("{}@example.com", username),
              "profile": {
                "first_name": first_name,
                "last_name": last_name
              }
            })
          })
          .collect();

        serde_json::json!({
          "success": true,
          "data": users,
          "timestamp": chrono::Utc::now().to_rfc3339(),
          "processing_time_ms": 23
        })
      }

      "analytics" => {
        serde_json::json!({
          "success": true,
          "data": {
            "overview": {
              "total_users": 5432,
              "active_users_today": 234,
              "page_views_today": 3421,
              "bounce_rate": 34.56
            },
            "traffic_sources": [
              {"source": "Direct", "visits": 543, "percentage": 32.1},
              {"source": "Google", "visits": 678, "percentage": 40.2},
              {"source": "Social Media", "visits": 321, "percentage": 19.0},
              {"source": "Referral", "visits": 145, "percentage": 8.7}
            ],
            "hourly_data": (0..24).map(|hour| {
              serde_json::json!({
                "hour": hour,
                "visits": 50 + (hour * 7) % 150,
                "unique_visitors": 30 + (hour * 5) % 100
              })
            }).collect::<Vec<_>>()
          },
          "timestamp": chrono::Utc::now().to_rfc3339(),
          "processing_time_ms": 12
        })
      }

      _ => {
        serde_json::json!({
          "success": false,
          "error": format!("Unknown request type: {}", request_type),
          "timestamp": chrono::Utc::now().to_rfc3339(),
          "processing_time_ms": 0
        })
      }
    };

    Some(result.to_string())
  }
}

// Convenience functions for creating processors and processing requests
pub fn create_v8_processor() -> Option<V8TypeScriptProcessor> {
  V8TypeScriptProcessor::new()
}

pub fn process_sample_requests() -> Vec<String> {
  let processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor - TypeScript files not found".to_string()],
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
  let processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor - TypeScript files not found".to_string()],
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

// Get information about the V8 code loading status
pub fn get_v8_code_status() -> String {
  match V8_CODE.lock() {
    Ok(guard) => match guard.as_ref() {
      Some(code) => format!(
        "✅ V8 TypeScript code loaded successfully\n📁 v8-processing.js: {} characters\n📁 \
         data-generators.js: {} characters\n🔄 Using once_cell for global storage",
        code.v8_processing_js.len(),
        code.data_generators_js.len()
      ),
      None => {
        "❌ V8 TypeScript code failed to load - check if client/dist/v8/ files exist".to_string()
      }
    },
    Err(_) => "❌ V8 TypeScript code mutex is poisoned".to_string(),
  }
}

// Force reload the V8 code (useful for development)
pub fn reload_v8_code() -> bool {
  match V8_CODE.lock() {
    Ok(mut guard) => {
      *guard = V8TypeScriptCode::new();
      guard.is_some()
    }
    Err(_) => false,
  }
}
