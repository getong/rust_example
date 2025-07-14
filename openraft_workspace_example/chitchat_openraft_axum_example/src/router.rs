use aide::axum::ApiRouter;

use crate::{
  api::{
    AppState, get_members_docs, get_raft_store_docs, get_raft_store_html_docs, get_state_docs,
    raft_get_docs, raft_list_tables_docs, raft_set_docs, update_service_docs,
  },
  docs::docs_routes,
};

pub fn create_router() -> ApiRouter<AppState> {
  tracing::error!("=== CREATING API ROUTER ===");

  let router = ApiRouter::new()
    .api_route("/", get_state_docs().into())
    .api_route("/members", get_members_docs().into())
    .api_route("/update_service", update_service_docs().into())
    .api_route("/raft/set", raft_set_docs().into())
    .api_route("/raft/get/{table}/{key}", raft_get_docs().into())
    .api_route("/raft/tables", raft_list_tables_docs().into())
    .api_route("/raft/store", get_raft_store_docs().into())
    .api_route("/raft/store/html", get_raft_store_html_docs().into())
    .route("/test", axum::routing::get(test_handler))
    .fallback(fallback_handler)
    .merge(docs_routes());

  tracing::error!("=== ROUTER CREATED WITH ROUTES ===");
  tracing::error!(
    "Routes: /, /members, /update_service, /raft/set, /raft/get/{{table}}/{{key}}, /raft/tables, \
     /raft/store, /raft/store/html, /test"
  );

  router
}

// Create a simple test router without aide
pub fn create_simple_router() -> axum::Router<AppState> {
  tracing::error!("=== CREATING SIMPLE ROUTER (NO AIDE) ===");

  axum::Router::new()
    .route("/", axum::routing::get(simple_handler))
    .route("/test", axum::routing::get(test_handler))
    .route(
      "/raft/store/html",
      axum::routing::get(simple_raft_store_html),
    )
    .route("/raft/store", axum::routing::get(simple_raft_store))
    .fallback(fallback_handler)
}

async fn test_handler() -> &'static str {
  tracing::error!("=== TEST HANDLER CALLED ===");
  "Test endpoint working!"
}

async fn fallback_handler(uri: axum::http::Uri) -> axum::response::Json<serde_json::Value> {
  tracing::error!("=== FALLBACK HANDLER CALLED ===");
  tracing::error!("Unmatched URI: {}", uri);
  tracing::error!("Path: {}", uri.path());
  tracing::error!("Query: {:?}", uri.query());

  axum::response::Json(serde_json::json!({
    "error": "Route not found",
    "path": uri.path(),
    "message": "This endpoint does not exist"
  }))
}

async fn simple_handler() -> &'static str {
  tracing::error!("=== SIMPLE HANDLER CALLED ===");
  "Simple handler working!"
}

async fn simple_raft_store_html() -> axum::response::Html<String> {
  tracing::error!("=== SIMPLE RAFT STORE HTML HANDLER CALLED ===");
  axum::response::Html("<h1>Simple OpenRAFT Store HTML</h1><p>This is working!</p>".to_string())
}

async fn simple_raft_store() -> axum::response::Json<serde_json::Value> {
  tracing::error!("=== SIMPLE RAFT STORE JSON HANDLER CALLED ===");
  axum::response::Json(serde_json::json!({
    "message": "Simple raft store endpoint working",
    "status": "ok"
  }))
}

// async fn serve_docs(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
//   Json(api)
// }
