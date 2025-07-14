use aide::{
  axum::{ApiRouter, IntoApiResponse},
  openapi::OpenApi,
  redoc::Redoc,
  scalar::Scalar,
  swagger::Swagger,
};
use axum::{Extension, Json, extract::Query, response::Html, routing::get};
use serde::Deserialize;

use crate::api::AppState;

#[derive(Deserialize)]
pub struct DocsQuery {
  url: Option<String>,
}

pub fn docs_routes() -> ApiRouter<AppState> {
  ApiRouter::new()
    .route("/docs", get(docs_index))
    .route("/docs/scalar", Scalar::new("/docs/api.json").axum_route())
    .route("/docs/swagger", Swagger::new("/docs/api.json").axum_route())
    .route("/docs/redoc", Redoc::new("/docs/api.json").axum_route())
    .route("/docs/api.json", get(serve_docs))
}

async fn serve_docs(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
  Json(api)
}

async fn docs_index(Query(query): Query<DocsQuery>) -> Html<String> {
  let docs_url = query.url.as_deref().unwrap_or("/docs/api.json");

  Html(format!(
    r#"
<!DOCTYPE html>
<html>
<head>
    <title>Chitchat Cluster API Documentation</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .header {{ text-align: center; margin-bottom: 40px; }}
        .links {{ display: flex; justify-content: center; gap: 20px; }}
        .link {{
            display: inline-block;
            padding: 10px 20px;
            background-color: #007bff;
            color: white;
            text-decoration: none;
            border-radius: 5px;
        }}
        .link:hover {{ background-color: #0056b3; }}
        .description {{
            max-width: 600px;
            margin: 0 auto 30px;
            text-align: center;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Chitchat Cluster API Documentation</h1>
        <p class="description">
            Welcome to the Chitchat Cluster API documentation.
            Choose your preferred documentation format below.
        </p>
    </div>

    <div class="links">
        <a href="/docs/scalar" class="link">Scalar UI</a>
        <a href="/docs/swagger" class="link">Swagger UI</a>
        <a href="/docs/redoc" class="link">Redoc</a>
        <a href="{}" class="link">Raw OpenAPI JSON</a>
    </div>
</body>
</html>
"#,
    docs_url
  ))
}
