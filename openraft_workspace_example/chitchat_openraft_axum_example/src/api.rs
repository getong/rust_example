use std::{net::SocketAddr, sync::Arc};

use aide::axum::routing::{get_with, post_with};
use axum::extract::{Json, Path, State};
use base64::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
  ApiResponse, ClusterMembersResponse, ServiceUpdateResponse,
  distributed::{
    Cluster,
    raft_types::{Key, Request, Response, Table, Value},
  },
  utils::create_service,
};

#[derive(Clone)]
pub struct AppState {
  pub cluster: Arc<Cluster>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceUpdateParams {
  /// The type of service to update
  pub service_type: String,
  /// The host address for the service
  pub host: String,
  /// Optional shard ID for sharded services
  pub shard: Option<u64>,
}

/// Parameters for OpenRAFT key-value operations
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaftSetRequest {
  /// The table name
  pub table: String,
  /// The key to set
  pub key: String,
  /// The value to set (base64 encoded)
  pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaftGetRequest {
  /// The table name
  pub table: String,
  /// The key to get
  pub key: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RaftResponse {
  /// Success status
  pub success: bool,
  /// Response message
  pub message: String,
  /// Optional data payload
  pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RaftStoreResponse {
  /// Success status
  pub success: bool,
  /// Response message
  pub message: String,
  /// Store statistics
  pub stats: RaftStoreStats,
  /// Table data
  pub tables: Vec<TableData>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RaftStoreStats {
  /// Total number of tables
  pub total_tables: usize,
  /// Total number of keys across all tables
  pub total_keys: usize,
  /// Last applied log ID
  pub last_applied: Option<String>,
  /// Current membership info
  pub membership_info: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TableData {
  /// Table name
  pub name: String,
  /// Number of keys in this table
  pub key_count: usize,
  /// Sample of key-value pairs (limited to first 10)
  pub sample_data: Vec<KeyValuePair>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct KeyValuePair {
  /// The key
  pub key: String,
  /// The value (base64 encoded)
  pub value: String,
  /// Value size in bytes
  pub size: usize,
}

/// Get the current chitchat cluster state
pub async fn get_state(State(state): State<AppState>) -> Json<ApiResponse> {
  let cluster_state = state.cluster.cluster_state().await;
  let live_nodes = state.cluster.live_nodes().await;
  let dead_nodes = state.cluster.dead_nodes().await;

  let response = ApiResponse {
    cluster_id: "chitchat-example-cluster".to_string(),
    cluster_state: serde_json::to_value(cluster_state).unwrap_or_default(),
    live_nodes: live_nodes.into_iter().map(|id| id.node_id).collect(),
    dead_nodes: dead_nodes.into_iter().map(|id| id.node_id).collect(),
  };
  Json(response)
}

/// Get cluster members with their services
pub async fn get_members(State(state): State<AppState>) -> Json<ClusterMembersResponse> {
  let members = state.cluster.members().await;
  Json(ClusterMembersResponse { members })
}

/// Update the service of the current node
pub async fn update_service(
  State(state): State<AppState>,
  Json(params): Json<ServiceUpdateParams>,
) -> Json<ServiceUpdateResponse> {
  let host: SocketAddr = match params.host.parse() {
    Ok(addr) => addr,
    Err(_) => {
      return Json(ServiceUpdateResponse {
        status: false,
        message: "Invalid host format".to_string(),
      });
    }
  };

  let service = create_service(&params.service_type, host, params.shard);

  match state.cluster.set_service(service).await {
    Ok(_) => Json(ServiceUpdateResponse {
      status: true,
      message: "Service updated successfully".to_string(),
    }),
    Err(e) => Json(ServiceUpdateResponse {
      status: false,
      message: format!("Failed to update service: {}", e),
    }),
  }
}

/// Set a key-value pair using OpenRAFT
pub async fn raft_set(
  State(state): State<AppState>,
  Json(params): Json<RaftSetRequest>,
) -> Json<RaftResponse> {
  let value_bytes = match base64::prelude::BASE64_STANDARD.decode(&params.value) {
    Ok(bytes) => bytes,
    Err(_) => {
      return Json(RaftResponse {
        success: false,
        message: "Invalid base64 value".to_string(),
        data: None,
      });
    }
  };

  let request = Request::Set {
    table: Table(params.table),
    key: Key(params.key),
    value: Value(value_bytes),
  };

  match state.cluster.raft_request(request).await {
    Ok(Response::Set(Ok(()))) => Json(RaftResponse {
      success: true,
      message: "Key set successfully".to_string(),
      data: None,
    }),
    Ok(Response::Set(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to set key: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
    }),
  }
}

/// Get a value by key using OpenRAFT
pub async fn raft_get(
  State(state): State<AppState>,
  Path((table, key)): Path<(String, String)>,
) -> Json<RaftResponse> {
  let request = Request::Get {
    table: Table(table),
    key: Key(key),
  };

  match state.cluster.raft_request(request).await {
    Ok(Response::Get(Ok(Some(value)))) => {
      let encoded_value = base64::prelude::BASE64_STANDARD.encode(&value.0);
      Json(RaftResponse {
        success: true,
        message: "Key found".to_string(),
        data: Some(serde_json::json!({ "value": encoded_value })),
      })
    }
    Ok(Response::Get(Ok(None))) => Json(RaftResponse {
      success: false,
      message: "Key not found".to_string(),
      data: None,
    }),
    Ok(Response::Get(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to get key: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
    }),
  }
}

/// List all tables using OpenRAFT
pub async fn raft_list_tables(State(state): State<AppState>) -> Json<RaftResponse> {
  let request = Request::AllTables;

  match state.cluster.raft_request(request).await {
    Ok(Response::AllTables(Ok(tables))) => {
      let table_names: Vec<String> = tables.into_iter().map(|t| t.0).collect();
      Json(RaftResponse {
        success: true,
        message: "Tables listed successfully".to_string(),
        data: Some(serde_json::json!({ "tables": table_names })),
      })
    }
    Ok(Response::AllTables(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to list tables: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
    }),
  }
}

/// Get OpenRAFT store data with HTML formatting
pub async fn get_raft_store_html(State(state): State<AppState>) -> axum::response::Html<String> {
  tracing::error!("=== RAFT STORE HTML HANDLER CALLED ===");
  tracing::error!("AppState cluster reference: {:p}", &*state.cluster);

  let store_data = get_raft_store(State(state)).await.0;

  tracing::error!(
    "Store data retrieved: success={}, tables_count={}",
    store_data.success,
    store_data.tables.len()
  );

  let html = format!(
    r#"
<!DOCTYPE html>
<html>
<head>
    <title>OpenRAFT Store Data</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; background-color: #f5f5f5; }}
        .header {{ background-color: #2c3e50; color: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; text-align: center; }}
        .stats {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin-bottom: 20px; }}
        .stat-card {{ background-color: #e8f4f8; padding: 15px; border-radius: 8px; text-align: center; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .stat-value {{ font-size: 24px; font-weight: bold; color: #2c3e50; }}
        .stat-label {{ font-size: 14px; color: #7f8c8d; margin-top: 5px; }}
        .table-section {{ margin-bottom: 30px; background-color: white; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .table-header {{ background-color: #3498db; color: white; padding: 15px; border-radius: 8px 8px 0 0; }}
        .table-content {{ border: 1px solid #ddd; border-top: none; border-radius: 0 0 8px 8px; }}
        .key-value {{ padding: 15px; border-bottom: 1px solid #eee; }}
        .key-value:last-child {{ border-bottom: none; }}
        .key {{ font-weight: bold; color: #2c3e50; margin-bottom: 5px; }}
        .value {{ color: #7f8c8d; font-family: monospace; word-break: break-all; background-color: #f8f9fa; padding: 8px; border-radius: 4px; margin-bottom: 5px; }}
        .size {{ color: #95a5a6; font-size: 12px; }}
        .error {{ color: #e74c3c; background-color: #fdf2f2; padding: 15px; border-radius: 8px; }}
        .success {{ color: #27ae60; }}
        .empty-state {{ text-align: center; padding: 40px; color: #7f8c8d; font-size: 18px; }}
        .refresh-btn {{ background-color: #3498db; color: white; padding: 10px 20px; border: none; border-radius: 4px; cursor: pointer; text-decoration: none; display: inline-block; margin: 10px; }}
        .refresh-btn:hover {{ background-color: #2980b9; }}
    </style>
    <script>
        function refreshPage() {{
            window.location.reload();
        }}

        // Auto-refresh every 30 seconds
        setTimeout(function() {{
            refreshPage();
        }}, 30000);
    </script>
</head>
<body>
    <div class="header">
        <h1>🗄️ OpenRAFT Distributed Store</h1>
        <p class="{}">Status: {}</p>
        <button class="refresh-btn" onclick="refreshPage()">🔄 Refresh</button>
    </div>

    <div class="stats">
        <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Total Tables</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Total Keys</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{}</div>
            <div class="stat-label">Last Applied</div>
        </div>
    </div>

    <div class="membership-info" style="background-color: white; padding: 15px; border-radius: 8px; margin-bottom: 20px; box-shadow: 0 2px 4px rgba(0,0,0,0.1);">
        <p><strong>Membership:</strong> {}</p>
    </div>

    {}

    <div style="margin-top: 30px; padding: 15px; background-color: white; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1);">
        <h3>🔗 API Endpoints</h3>
        <ul>
            <li><a href="/raft/store" target="_blank">📊 JSON API</a> - Raw JSON data</li>
            <li><a href="/raft/tables" target="_blank">📋 List Tables</a> - Get all table names</li>
            <li><strong>POST</strong> <code>/raft/set</code> - Set Key-Value pair</li>
            <li><strong>GET</strong> <code>/raft/get/{{table}}/{{key}}</code> - Get Value by key</li>
        </ul>

        <h4>📝 Example Commands:</h4>
        <pre style="background-color: #f8f9fa; padding: 10px; border-radius: 4px; overflow-x: auto;">
# Set a key-value pair
curl -X POST http://localhost:10000/raft/set -H "Content-Type: application/json" -d '{{"table":"users","key":"john","value":"{}"}}'

# Get a value
curl http://localhost:10000/raft/get/users/john

# List all tables
curl http://localhost:10000/raft/tables
        </pre>
    </div>

    <div style="text-align: center; margin-top: 20px; color: #7f8c8d; font-size: 12px;">
        Last updated: {} | Auto-refresh in 30s
    </div>
</body>
</html>
"#,
    if store_data.success {
      "success"
    } else {
      "error"
    },
    store_data.message,
    store_data.stats.total_tables,
    store_data.stats.total_keys,
    store_data.stats.last_applied.as_deref().unwrap_or("None"),
    store_data.stats.membership_info,
    generate_tables_html(&store_data.tables),
    base64::prelude::BASE64_STANDARD.encode("Hello World!"),
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
  );

  tracing::error!("=== RETURNING HTML RESPONSE ===");
  tracing::error!("HTML response length: {} bytes", html.len());
  axum::response::Html(html)
}

/// Get OpenRAFT store data and statistics
pub async fn get_raft_store(State(state): State<AppState>) -> Json<RaftStoreResponse> {
  tracing::error!("=== RAFT STORE JSON HANDLER CALLED ===");

  // First, get all tables
  let tables_request = Request::AllTables;
  let tables_result = match state.cluster.raft_request(tables_request).await {
    Ok(Response::AllTables(Ok(tables))) => tables,
    Ok(Response::AllTables(Err(e))) => {
      return Json(RaftStoreResponse {
        success: false,
        message: format!("Failed to get tables: {}", e),
        stats: RaftStoreStats {
          total_tables: 0,
          total_keys: 0,
          last_applied: None,
          membership_info: "Unknown".to_string(),
        },
        tables: vec![],
      });
    }
    Ok(_) => {
      return Json(RaftStoreResponse {
        success: false,
        message: "Unexpected response type for tables".to_string(),
        stats: RaftStoreStats {
          total_tables: 0,
          total_keys: 0,
          last_applied: None,
          membership_info: "Unknown".to_string(),
        },
        tables: vec![],
      });
    }
    Err(e) => {
      return Json(RaftStoreResponse {
        success: false,
        message: format!("Raft request failed: {}", e),
        stats: RaftStoreStats {
          total_tables: 0,
          total_keys: 0,
          last_applied: None,
          membership_info: "Unknown".to_string(),
        },
        tables: vec![],
      });
    }
  };

  let mut table_data = Vec::new();
  let mut total_keys = 0;

  // Get data for each table
  for table in &tables_result {
    // For demonstration, we'll get all keys by trying some common patterns
    // In a real implementation, you'd want a proper "list keys" operation
    let sample_keys = get_sample_keys_for_table(table, &state.cluster).await;

    let mut key_value_pairs = Vec::new();
    for key in sample_keys.iter().take(10) {
      // Limit to first 10 keys
      let get_request = Request::Get {
        table: table.clone(),
        key: key.clone(),
      };

      if let Ok(Response::Get(Ok(Some(value)))) = state.cluster.raft_request(get_request).await {
        let encoded_value = BASE64_STANDARD.encode(&value.0);
        key_value_pairs.push(KeyValuePair {
          key: key.0.clone(),
          value: encoded_value,
          size: value.0.len(),
        });
      }
    }

    total_keys += sample_keys.len();

    table_data.push(TableData {
      name: table.0.clone(),
      key_count: sample_keys.len(),
      sample_data: key_value_pairs,
    });
  }

  // Get store statistics from the state machine if available
  let (last_applied, membership_info) = if let Some(store) = state.cluster.raft_store().await {
    let sm = store.state_machine.lock().unwrap();
    let last_applied = sm.last_applied.map(|id| {
      format!(
        "T{}-N{}.{}",
        id.committed_leader_id(),
        id.committed_leader_id(),
        id.index()
      )
    });
    let membership_info = format!("Membership: {:?}", sm.last_membership);
    (last_applied, membership_info)
  } else {
    (None, "OpenRAFT not enabled".to_string())
  };

  let stats = RaftStoreStats {
    total_tables: tables_result.len(),
    total_keys,
    last_applied,
    membership_info,
  };

  Json(RaftStoreResponse {
    success: true,
    message: "Store data retrieved successfully".to_string(),
    stats,
    tables: table_data,
  })
}

/// Helper function to get sample keys for a table
/// This is a simplified approach - in a real implementation you'd want proper key listing
async fn get_sample_keys_for_table(table: &Table, cluster: &Cluster) -> Vec<Key> {
  let mut keys = Vec::new();

  // Try some common key patterns for demonstration
  let common_patterns = vec![
    "key1", "key2", "key3", "test", "example", "data", "config", "user", "session", "cache",
  ];

  for pattern in common_patterns {
    let key = Key(pattern.to_string());
    let get_request = Request::Get {
      table: table.clone(),
      key: key.clone(),
    };

    if let Ok(Response::Get(Ok(Some(_)))) = cluster.raft_request(get_request).await {
      keys.push(key);
    }
  }

  keys
}

fn generate_tables_html(tables: &[TableData]) -> String {
  if tables.is_empty() {
    return r#"
    <div class="empty-state">
        <h3>📭 No Tables Found</h3>
        <p>The OpenRAFT store is empty. Try adding some data first!</p>
        <p>Use the API endpoints below to add key-value pairs.</p>
    </div>
    "#
    .to_string();
  }

  let mut html = String::new();

  for table in tables {
    html.push_str(&format!(
      r#"
    <div class="table-section">
        <div class="table-header">
            <h3>📁 Table: {} ({} keys)</h3>
        </div>
        <div class="table-content">
"#,
      table.name, table.key_count
    ));

    if table.sample_data.is_empty() {
      html.push_str(r#"<div class="key-value"><div class="empty-state">No data available in this table</div></div>"#);
    } else {
      for kv in &table.sample_data {
        html.push_str(&format!(
          r#"
            <div class="key-value">
                <div class="key">🔑 {}</div>
                <div class="value">{}</div>
                <div class="size">📏 {} bytes</div>
            </div>
"#,
          html_escape(&kv.key),
          html_escape(&kv.value[.. kv.value.len().min(200)]), // Show more characters
          kv.size
        ));
      }
    }

    html.push_str("        </div>\n    </div>");
  }

  html
}

fn html_escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

pub fn get_state_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(get_state, |op| {
    op.summary("Get cluster state")
      .description(
        "Returns the current state of the chitchat cluster including live and dead nodes",
      )
      .response::<200, ApiResponse>()
  })
}

pub fn get_members_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(get_members, |op| {
    op.summary("Get cluster members")
      .description("Returns all members in the cluster with their service information")
      .response::<200, ClusterMembersResponse>()
  })
}

pub fn update_service_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  post_with(update_service, |op| {
    op.summary("Update service")
      .description("Updates the service configuration for the current node")
      .response::<200, ServiceUpdateResponse>()
  })
}

pub fn raft_set_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  post_with(raft_set, |op| {
    op.summary("Set key-value using OpenRAFT")
      .description("Sets a key-value pair in the distributed store using OpenRAFT consensus")
      .response::<200, Json<RaftResponse>>()
  })
}

pub fn raft_get_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(raft_get, |op| {
    op.summary("Get value by key using OpenRAFT")
      .description("Retrieves a value by key from the distributed store")
      .response::<200, Json<RaftResponse>>()
  })
}

pub fn raft_list_tables_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(raft_list_tables, |op| {
    op.summary("List all tables using OpenRAFT")
      .description("Lists all tables in the distributed store")
      .response::<200, Json<RaftResponse>>()
  })
}

pub fn get_raft_store_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  tracing::error!("=== CREATING RAFT STORE JSON DOCS ROUTE ===");
  get_with(get_raft_store, |op| {
    op.summary("Get OpenRAFT store data")
      .description(
        "Retrieves all data from the OpenRAFT distributed store including tables, keys, and \
         statistics",
      )
      .response::<200, Json<RaftStoreResponse>>()
  })
}

pub fn get_raft_store_html_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  tracing::error!("=== CREATING RAFT STORE HTML DOCS ROUTE ===");
  get_with(get_raft_store_html, |op| {
    op.summary("Get OpenRAFT store data (HTML)")
      .description("Retrieves OpenRAFT store data formatted as HTML for browser viewing")
      .response::<200, axum::response::Html<String>>()
  })
}
