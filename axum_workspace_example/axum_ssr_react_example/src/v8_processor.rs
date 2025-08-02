use std::{fs, sync::Mutex};

use once_cell::sync::Lazy;

// use ssr_rs::v8;
use crate::config::STREAM_CONFIG;

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
  pub jsonplaceholder_demo_js: String,
  pub stream_chat_demo_js: String,
}

impl V8TypeScriptCode {
  pub fn new() -> Option<Self> {
    // Load the compiled TypeScript files
    match (
      fs::read_to_string("client/dist/v8/v8-processing.js"),
      fs::read_to_string("client/dist/v8/data-generators.js"),
      fs::read_to_string("client/dist/v8/jsonplaceholder-demo.js"),
      fs::read_to_string("client/dist/v8/stream-chat-demo.js"),
    ) {
      (
        Ok(v8_processing_js),
        Ok(data_generators_js),
        Ok(jsonplaceholder_demo_js),
        Ok(stream_chat_demo_js),
      ) => Some(Self {
        v8_processing_js,
        data_generators_js,
        jsonplaceholder_demo_js,
        stream_chat_demo_js,
      }),
      _ => None,
    }
  }
}

// Global V8 code instance using once_cell
static V8_CODE: Lazy<Mutex<Option<V8TypeScriptCode>>> =
  Lazy::new(|| Mutex::new(V8TypeScriptCode::new()));

// V8 TypeScript processor that works with ssr_rs V8 runtime
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

  // Execute Stream Chat TypeScript code with real credentials
  pub fn execute_stream_chat_with_credentials(
    &mut self,
    request_type: &str,
    params: Option<serde_json::Value>,
  ) -> Option<String> {
    // Create params with real Stream Chat credentials
    let mut enhanced_params = params.unwrap_or(serde_json::json!({}));
    if let Some(obj) = enhanced_params.as_object_mut() {
      obj.insert(
        "api_key".to_string(),
        serde_json::json!(&STREAM_CONFIG.api_key),
      );
      obj.insert(
        "api_secret".to_string(),
        serde_json::json!(&STREAM_CONFIG.api_secret),
      );
    }

    // Use v8_stream_executor to execute the TypeScript code
    match crate::v8_stream_executor::StreamChatExecutor::execute_function(
      request_type,
      enhanced_params,
    ) {
      Ok(result) => Some(result),
      Err(e) => {
        eprintln!("Failed to execute Stream Chat function: {}", e);
        None
      }
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

  // JSONPlaceholder API simulation methods
  pub fn fetch_jsonplaceholder_data(&self, endpoint: &str, id: Option<i64>) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    // Simulate the TypeScript fetchJsonPlaceholderData function
    let result = match endpoint {
      "posts" => {
        if let Some(post_id) = id {
          // Single post
          match post_id {
            1 => serde_json::json!({
              "success": true,
              "data": {
                "userId": 1,
                "id": 1,
                "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
                "body": "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
              },
              "metadata": {
                "endpoint": format!("posts/{}", post_id),
                "returned_count": 1,
                "total_available": 100,
                "api_source": "jsonplaceholder.typicode.com (simulated)",
                "cached": false
              },
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 12
            }),
            2 => serde_json::json!({
              "success": true,
              "data": {
                "userId": 1,
                "id": 2,
                "title": "qui est esse",
                "body": "est rerum tempore vitae\nsequi sint nihil reprehenderit dolor beatae ea dolores neque\nfugiat blanditiis voluptate porro vel nihil molestiae ut reiciendis\nqui aperiam non debitis possimus qui neque nisi nulla"
              },
              "metadata": {
                "endpoint": format!("posts/{}", post_id),
                "returned_count": 1,
                "total_available": 100,
                "api_source": "jsonplaceholder.typicode.com (simulated)",
                "cached": true
              },
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 8
            }),
            _ => serde_json::json!({
              "success": false,
              "error": format!("Post with id {} not found", post_id),
              "endpoint": format!("posts/{}", post_id),
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 5
            }),
          }
        } else {
          // All posts
          serde_json::json!({
            "success": true,
            "data": [
              {
                "userId": 1,
                "id": 1,
                "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
                "body": "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
              },
              {
                "userId": 1,
                "id": 2,
                "title": "qui est esse",
                "body": "est rerum tempore vitae\nsequi sint nihil reprehenderit dolor beatae ea dolores neque\nfugiat blanditiis voluptate porro vel nihil molestiae ut reiciendis\nqui aperiam non debitis possimus qui neque nisi nulla"
              },
              {
                "userId": 2,
                "id": 3,
                "title": "ea molestias quasi exercitationem repellat qui ipsa sit aut",
                "body": "et iusto sed quo iure\nvoluptatem occaecati omnis eligendi aut ad\nvoluptatem doloribus vel accusantium quis pariatur\nmolestiae porro eius odio et labore et velit aut"
              }
            ],
            "metadata": {
              "endpoint": "posts",
              "returned_count": 3,
              "total_available": 100,
              "api_source": "jsonplaceholder.typicode.com (simulated)",
              "cached": false
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "processing_time_ms": 23
          })
        }
      }

      "users" => {
        if let Some(user_id) = id {
          // Single user
          match user_id {
            1 => serde_json::json!({
              "success": true,
              "data": {
                "id": 1,
                "name": "Leanne Graham",
                "username": "Bret",
                "email": "Sincere@april.biz",
                "address": {
                  "street": "Kulas Light",
                  "suite": "Apt. 556",
                  "city": "Gwenborough",
                  "zipcode": "92998-3874",
                  "geo": {
                    "lat": "-37.3159",
                    "lng": "81.1496"
                  }
                },
                "phone": "1-770-736-8031 x56442",
                "website": "hildegard.org",
                "company": {
                  "name": "Romaguera-Crona",
                  "catchPhrase": "Multi-layered client-server neural-net",
                  "bs": "harness real-time e-markets"
                }
              },
              "metadata": {
                "endpoint": format!("users/{}", user_id),
                "returned_count": 1,
                "total_available": 10,
                "api_source": "jsonplaceholder.typicode.com (simulated)",
                "cached": false
              },
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 15
            }),
            2 => serde_json::json!({
              "success": true,
              "data": {
                "id": 2,
                "name": "Ervin Howell",
                "username": "Antonette",
                "email": "Shanna@melissa.tv",
                "address": {
                  "street": "Victor Plains",
                  "suite": "Suite 879",
                  "city": "Wisokyburgh",
                  "zipcode": "90566-7771",
                  "geo": {
                    "lat": "-43.9509",
                    "lng": "-34.4618"
                  }
                },
                "phone": "010-692-6593 x09125",
                "website": "anastasia.net",
                "company": {
                  "name": "Deckow-Crist",
                  "catchPhrase": "Proactive didactic contingency",
                  "bs": "synergize scalable supply-chains"
                }
              },
              "metadata": {
                "endpoint": format!("users/{}", user_id),
                "returned_count": 1,
                "total_available": 10,
                "api_source": "jsonplaceholder.typicode.com (simulated)",
                "cached": true
              },
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 9
            }),
            _ => serde_json::json!({
              "success": false,
              "error": format!("User with id {} not found", user_id),
              "endpoint": format!("users/{}", user_id),
              "timestamp": chrono::Utc::now().to_rfc3339(),
              "processing_time_ms": 3
            }),
          }
        } else {
          // All users (limited sample)
          serde_json::json!({
            "success": true,
            "data": [
              {
                "id": 1,
                "name": "Leanne Graham",
                "username": "Bret",
                "email": "Sincere@april.biz"
              },
              {
                "id": 2,
                "name": "Ervin Howell",
                "username": "Antonette",
                "email": "Shanna@melissa.tv"
              }
            ],
            "metadata": {
              "endpoint": "users",
              "returned_count": 2,
              "total_available": 10,
              "api_source": "jsonplaceholder.typicode.com (simulated)",
              "cached": false
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "processing_time_ms": 18
          })
        }
      }

      "todos" => {
        serde_json::json!({
          "success": true,
          "data": [
            {"userId": 1, "id": 1, "title": "delectus aut autem", "completed": false},
            {"userId": 1, "id": 2, "title": "quis ut nam facilis et officia qui", "completed": false},
            {"userId": 1, "id": 3, "title": "fugiat veniam minus", "completed": false},
            {"userId": 2, "id": 4, "title": "et porro tempora", "completed": true}
          ],
          "metadata": {
            "endpoint": "todos",
            "returned_count": 4,
            "total_available": 200,
            "api_source": "jsonplaceholder.typicode.com (simulated)",
            "cached": false
          },
          "timestamp": chrono::Utc::now().to_rfc3339(),
          "processing_time_ms": 14
        })
      }

      _ => serde_json::json!({
        "success": false,
        "error": format!("Unknown endpoint: {}", endpoint),
        "available_endpoints": ["posts", "users", "comments", "albums", "photos", "todos"],
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "processing_time_ms": 2
      }),
    };

    Some(result.to_string())
  }

  pub fn get_user_posts(&self, user_id: i64) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    let result = match user_id {
      1 => serde_json::json!({
        "success": true,
        "data": {
          "user": {
            "id": 1,
            "name": "Leanne Graham",
            "username": "Bret",
            "email": "Sincere@april.biz"
          },
          "posts": [
            {
              "userId": 1,
              "id": 1,
              "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
              "body": "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
            },
            {
              "userId": 1,
              "id": 2,
              "title": "qui est esse",
              "body": "est rerum tempore vitae\nsequi sint nihil reprehenderit dolor beatae ea dolores neque\nfugiat blanditiis voluptate porro vel nihil molestiae ut reiciendis\nqui aperiam non debitis possimus qui neque nisi nulla"
            }
          ],
          "stats": {
            "total_posts": 2,
            "avg_body_length": 186
          }
        },
        "metadata": {
          "endpoint": format!("users/{}/posts", user_id),
          "processing_type": "aggregated_data",
          "api_source": "jsonplaceholder.typicode.com (simulated)"
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "processing_time_ms": 28
      }),
      2 => serde_json::json!({
        "success": true,
        "data": {
          "user": {
            "id": 2,
            "name": "Ervin Howell",
            "username": "Antonette",
            "email": "Shanna@melissa.tv"
          },
          "posts": [
            {
              "userId": 2,
              "id": 3,
              "title": "ea molestias quasi exercitationem repellat qui ipsa sit aut",
              "body": "et iusto sed quo iure\nvoluptatem occaecati omnis eligendi aut ad\nvoluptatem doloribus vel accusantium quis pariatur\nmolestiae porro eius odio et labore et velit aut"
            }
          ],
          "stats": {
            "total_posts": 1,
            "avg_body_length": 174
          }
        },
        "metadata": {
          "endpoint": format!("users/{}/posts", user_id),
          "processing_type": "aggregated_data",
          "api_source": "jsonplaceholder.typicode.com (simulated)"
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "processing_time_ms": 22
      }),
      _ => serde_json::json!({
        "success": false,
        "error": format!("User with id {} not found", user_id),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "processing_time_ms": 5
      }),
    };

    Some(result.to_string())
  }

  pub fn analyze_jsonplaceholder_data(&self) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    let result = serde_json::json!({
      "success": true,
      "data": {
        "posts": {
          "total": 3,
          "avg_title_length": 51,
          "avg_body_length": 181,
          "posts_by_user": {
            "1": 2,
            "2": 1
          }
        },
        "users": {
          "total": 2,
          "domains": {
            "april.biz": 1,
            "melissa.tv": 1
          },
          "cities": ["Gwenborough", "Wisokyburgh"]
        },
        "comments": {
          "total": 2,
          "avg_body_length": 156
        },
        "todos": {
          "total": 4,
          "completed": 1,
          "completion_rate": 25
        }
      },
      "metadata": {
        "endpoint": "analytics/overview",
        "analysis_type": "comprehensive_stats",
        "sample_size": {
          "posts": 3,
          "users": 2,
          "comments": 2,
          "todos": 4
        }
      },
      "timestamp": chrono::Utc::now().to_rfc3339(),
      "processing_time_ms": 35
    });

    Some(result.to_string())
  }

  // Stream Chat processing methods - Using proper Stream Chat API pattern
  pub fn authenticate_stream_user(
    &mut self,
    user_id: &str,
    api_key: Option<&str>,
    api_secret: Option<&str>,
  ) -> Option<String> {
    // Use real Stream Chat credentials from environment variables
    let api_key = api_key.unwrap_or(&STREAM_CONFIG.api_key);
    let api_secret = api_secret.unwrap_or(&STREAM_CONFIG.api_secret);

    // Call TypeScript authenticateUser function
    let params = serde_json::json!({
      "user_id": user_id,
      "api_key": api_key,
      "api_secret": api_secret
    });

    self.execute_stream_chat_with_credentials("authenticate", Some(params))
  }

  pub fn get_user_chat_context(&mut self, user_id: &str) -> Option<String> {
    let params = serde_json::json!({
      "user_id": user_id,
      "api_key": &STREAM_CONFIG.api_key,
      "api_secret": &STREAM_CONFIG.api_secret
    });

    self.execute_stream_chat_with_credentials("user-context", Some(params))
  }

  pub fn analyze_stream_chat_data(&mut self) -> Option<String> {
    let params = serde_json::json!({
      "api_key": &STREAM_CONFIG.api_key,
      "api_secret": &STREAM_CONFIG.api_secret
    });

    self.execute_stream_chat_with_credentials("analytics", Some(params))
  }

  pub fn get_stream_chat_demo_setup(&mut self) -> Option<String> {
    let params = serde_json::json!({
      "api_key": &STREAM_CONFIG.api_key,
      "api_secret": &STREAM_CONFIG.api_secret
    });

    self.execute_stream_chat_with_credentials("setup", Some(params))
  }
}

// Old simulation implementations (kept for reference)
impl V8TypeScriptProcessor {
  pub fn get_user_chat_context_old(&self, user_id: &str) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    let result = match user_id {
      "john" => serde_json::json!({
        "success": true,
        "data": {
          "user": {
            "id": "john",
            "name": "John Doe",
            "email": "john@example.com",
            "role": "admin"
          },
          "channels": [
            {
              "id": "general",
              "type": "messaging",
              "name": "General Discussion",
              "members": ["john", "jane", "bob", "alice"],
              "created_by": "john",
              "recent_messages": [
                {
                  "id": "msg1",
                  "text": "Welcome to the team chat! 🎉",
                  "user": { "id": "john", "name": "John Doe" },
                  "created_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 2 * 60 * 60, 0)
                    .unwrap_or_else(|| chrono::Utc::now())
                    .to_rfc3339()
                }
              ],
              "unread_count": 2,
              "last_message_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 30 * 60, 0)
                .unwrap_or_else(|| chrono::Utc::now())
                .to_rfc3339()
            },
            {
              "id": "engineering",
              "type": "team",
              "name": "Engineering Team",
              "members": ["john", "jane"],
              "created_by": "john",
              "recent_messages": [
                {
                  "id": "msg2",
                  "text": "Let's review the new API design",
                  "user": { "id": "jane", "name": "Jane Smith" },
                  "created_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 60 * 60, 0)
                    .unwrap_or_else(|| chrono::Utc::now())
                    .to_rfc3339()
                }
              ],
              "unread_count": 0,
              "last_message_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 60 * 60, 0)
                .unwrap_or_else(|| chrono::Utc::now())
                .to_rfc3339()
            }
          ],
          "stats": {
            "total_channels": 2,
            "total_messages": 2,
            "unread_messages": 2,
            "online_status": "online"
          }
        },
        "metadata": {
          "api_version": "v1.0",
          "server_time": chrono::Utc::now().to_rfc3339(),
          "rate_limit": {
            "remaining": 998,
            "reset_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() + 60 * 60, 0)
              .unwrap_or_else(|| chrono::Utc::now())
              .to_rfc3339()
          }
        },
        "processing_time_ms": 28
      }),
      "jane" => serde_json::json!({
        "success": true,
        "data": {
          "user": {
            "id": "jane",
            "name": "Jane Smith",
            "email": "jane@example.com",
            "role": "moderator"
          },
          "channels": [
            {
              "id": "general",
              "type": "messaging",
              "name": "General Discussion",
              "members": ["john", "jane", "bob", "alice"],
              "unread_count": 1,
              "last_message_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 15 * 60, 0)
                .unwrap_or_else(|| chrono::Utc::now())
                .to_rfc3339()
            },
            {
              "id": "engineering",
              "type": "team",
              "name": "Engineering Team",
              "members": ["john", "jane"],
              "unread_count": 0,
              "last_message_at": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 60 * 60, 0)
                .unwrap_or_else(|| chrono::Utc::now())
                .to_rfc3339()
            }
          ],
          "stats": {
            "total_channels": 2,
            "total_messages": 1,
            "unread_messages": 1,
            "online_status": "online"
          }
        },
        "processing_time_ms": 22
      }),
      _ => serde_json::json!({
        "success": false,
        "error": format!("User '{}' not found", user_id),
        "processing_time_ms": 3
      }),
    };

    Some(result.to_string())
  }

  pub fn analyze_stream_chat_data_old(&self) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    let result = serde_json::json!({
      "success": true,
      "data": {
        "users": {
          "total": 4,
          "by_role": {
            "admin": 1,
            "moderator": 1,
            "user": 2
          },
          "by_department": {
            "Engineering": 1,
            "Design": 1,
            "Marketing": 1,
            "Sales": 1
          }
        },
        "channels": {
          "total": 3,
          "by_type": {
            "messaging": 2,
            "team": 1
          },
          "by_category": {
            "public": 2,
            "private": 1
          },
          "avg_members": 3
        },
        "messages": {
          "total": 3,
          "avg_length": 28,
          "recent_activity": chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() - 60 * 60, 0)
            .unwrap_or_else(|| chrono::Utc::now())
            .to_rfc3339()
        },
        "engagement": {
          "active_users_today": 3,
          "messages_today": 87,
          "peak_online_users": 3
        }
      },
      "metadata": {
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "config": {
          "api_key": "demo_api_...",
          "base_url": "https://chat.stream-io-api.com"
        }
      },
      "processing_time_ms": 35
    });

    Some(result.to_string())
  }

  pub fn get_stream_chat_demo_setup_old(&self) -> Option<String> {
    let code_guard = V8_CODE.lock().ok()?;
    let _code = code_guard.as_ref()?;

    // Following Stream.io documentation pattern for setup
    let result = serde_json::json!({
      "success": true,
      "data": {
        "config": {
          "api_key": &STREAM_CONFIG.api_key,
          "api_secret": &STREAM_CONFIG.api_secret,
          "base_url": "https://chat.stream-io-api.com",
          "server_client_initialized": true,
          "authentication_enabled": true,
          "development_mode": false
        },
        "token_generation": {
          "method": "serverClient.createToken(user_id)",
          "includes_iat_claim": true,
          "default_expiration": "24 hours",
          "jwt_structure": {
            "header": {"alg": "HS256", "typ": "JWT"},
            "payload_includes": ["user_id", "iat", "exp"],
            "signature": "HMAC SHA256"
          }
        },
        "sample_users": [
          {
            "id": "john",
            "name": "John Doe",
            "role": "admin",
            "email": "john@example.com",
            "department": "Engineering"
          },
          {
            "id": "jane",
            "name": "Jane Smith",
            "role": "moderator",
            "email": "jane@example.com",
            "department": "Design"
          },
          {
            "id": "bob",
            "name": "Bob Wilson",
            "role": "user",
            "email": "bob@example.com",
            "department": "Marketing"
          },
          {
            "id": "alice",
            "name": "Alice Johnson",
            "role": "user",
            "email": "alice@example.com",
            "department": "Sales"
          }
        ],
        "sample_channels": [
          {
            "id": "general",
            "name": "General Discussion",
            "type": "messaging",
            "members": ["john", "jane", "bob", "alice"],
            "created_by": "john"
          },
          {
            "id": "engineering",
            "name": "Engineering Team",
            "type": "team",
            "members": ["john", "jane"],
            "created_by": "john"
          },
          {
            "id": "random",
            "name": "Random Chat",
            "type": "messaging",
            "members": ["jane", "bob", "alice"],
            "created_by": "jane"
          }
        ],
        "available_endpoints": [
          {
            "name": "authenticate",
            "description": "Generate user token using StreamChat.getInstance(api_key, api_secret).createToken(user_id)",
            "example": "/stream-chat/authenticate?user_id=john"
          },
          {
            "name": "user_context",
            "description": "Get user channels and messages",
            "example": "/stream-chat/user-context?user_id=john"
          },
          {
            "name": "analytics",
            "description": "Chat usage statistics",
            "example": "/stream-chat/analytics"
          },
          {
            "name": "demo_setup",
            "description": "Configuration information",
            "example": "/stream-chat/demo-setup"
          }
        ],
        "integration_example": {
          "server_side": {
            "initialize": "const serverClient = StreamChat.getInstance(api_key, api_secret);",
            "create_token": "const token = serverClient.createToken(user_id);",
            "with_expiration": "const token = serverClient.createToken(user_id, expireTime);",
            "with_iat": "const token = serverClient.createToken(user_id, expireTime, issuedAt);"
          },
          "client_side": {
            "connect_user": "await client.connectUser(userObject, tokenFromServer);",
            "token_provider": "client.connectUser(userObject, async () => { return await fetchTokenFromBackend(); });"
          }
        }
      },
      "timestamp": chrono::Utc::now().to_rfc3339(),
      "processing_time_ms": 8
    });

    Some(result.to_string())
  }
}

// Convenience functions for creating processors and processing requests
// pub fn create_v8_processor() -> Option<V8TypeScriptProcessor> {
//   V8TypeScriptProcessor::new()
// }

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

// JSONPlaceholder processing functions
pub fn process_jsonplaceholder_samples() -> Vec<String> {
  let processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor - TypeScript files not found".to_string()],
  };

  vec![
    processor
      .fetch_jsonplaceholder_data("posts", None)
      .unwrap_or_default(),
    processor
      .fetch_jsonplaceholder_data("posts", Some(1))
      .unwrap_or_default(),
    processor
      .fetch_jsonplaceholder_data("users", Some(2))
      .unwrap_or_default(),
    processor
      .fetch_jsonplaceholder_data("todos", None)
      .unwrap_or_default(),
    processor.get_user_posts(1).unwrap_or_default(),
    processor.analyze_jsonplaceholder_data().unwrap_or_default(),
  ]
}

// Stream Chat processing functions
pub fn process_stream_chat_samples() -> Vec<String> {
  let mut processor = match V8TypeScriptProcessor::new() {
    Some(p) => p,
    None => return vec!["Failed to create V8 processor - TypeScript files not found".to_string()],
  };

  vec![
    processor.get_stream_chat_demo_setup().unwrap_or_default(),
    processor
      .authenticate_stream_user("john", None, None)
      .unwrap_or_default(),
    processor
      .authenticate_stream_user("jane", None, None)
      .unwrap_or_default(),
    processor.get_user_chat_context("john").unwrap_or_default(),
    processor.get_user_chat_context("jane").unwrap_or_default(),
    processor.analyze_stream_chat_data().unwrap_or_default(),
  ]
}

// Get information about the V8 code loading status
pub fn get_v8_code_status() -> String {
  match V8_CODE.lock() {
    Ok(guard) => match guard.as_ref() {
      Some(code) => format!(
        "✅ V8 TypeScript code loaded successfully\n📁 v8-processing.js: {} characters\n📁 \
         data-generators.js: {} characters\n📁 jsonplaceholder-demo.js: {} characters\n🔄 Using \
         once_cell for global storage",
        code.v8_processing_js.len(),
        code.data_generators_js.len(),
        code.jsonplaceholder_demo_js.len()
      ),
      None => {
        "❌ V8 TypeScript code failed to load - check if client/dist/v8/ files exist".to_string()
      }
    },
    Err(_) => "❌ V8 TypeScript code mutex is poisoned".to_string(),
  }
}

// Force reload the V8 code (useful for development)
// pub fn reload_v8_code() -> bool {
//   match V8_CODE.lock() {
//     Ok(mut guard) => {
//       *guard = V8TypeScriptCode::new();
//       guard.is_some()
//     }
//     Err(_) => false,
//   }
// }
