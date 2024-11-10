// curl -X POST \
//     -H "Content-Type: image/jpeg" \
//     --data-binary @"$HOME/hello.jpg" \
//     http://127.0.0.1:3003/upload
use axum::{
  async_trait,
  body::Bytes,
  extract::{FromRequest, Multipart, Request},
  http::{header::CONTENT_TYPE, StatusCode},
  response::IntoResponse,
  routing::post,
  Router,
};
use tokio::io::AsyncWriteExt;
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

pub struct Jpeg(Bytes);

#[async_trait]
impl<S> FromRequest<S> for Jpeg
where
  Bytes: FromRequest<S>,
  S: Send + Sync,
{
  type Rejection = StatusCode;

  async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
    let Some(content_type) = req.headers().get(CONTENT_TYPE) else {
      return Err(StatusCode::BAD_REQUEST);
    };

    let body = if content_type == "multipart/form-data" {
      let mut multipart = Multipart::from_request(req, state)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

      let Ok(Some(field)) = multipart.next_field().await else {
        return Err(StatusCode::BAD_REQUEST);
      };

      field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?
    } else if content_type == "image/jpeg" {
      Bytes::from_request(req, state)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    } else {
      return Err(StatusCode::BAD_REQUEST);
    };

    Ok(Self(body))
  }
}

pub async fn upload_jpeg3(jpeg: Jpeg) -> impl IntoResponse {
  // Write the image bytes to a file
  let mut file = tokio::fs::File::create("upload.jpg").await.unwrap();
  file.write_all(&jpeg.0).await.unwrap();
  (StatusCode::CREATED, "image uploaded".to_string())
}

#[tokio::main]
async fn main() {
  let app = Router::new()
    .route("/upload", post(upload_jpeg3))
    // Add the request body size limit layer
    .layer(ServiceBuilder::new().layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))); // Set limit to 10 MB

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3003").await.unwrap();
  axum::serve(listener, app).await.unwrap();
}
