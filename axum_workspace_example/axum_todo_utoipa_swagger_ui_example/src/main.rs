use std::{
  io::Error,
  net::{Ipv4Addr, SocketAddr},
};

use tokio::net::TcpListener;
use utoipa::{
  Modify, OpenApi,
  openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_scalar::{Scalar, Servable as ScalarServable};
use utoipa_swagger_ui::SwaggerUi;

mod todo;

const TODO_TAG: &str = "todo";

#[tokio::main]
async fn main() -> Result<(), Error> {
  #[derive(OpenApi)]
  #[openapi(
        modifiers(&SecurityAddon),
        tags(
            (name = TODO_TAG, description = "Todo items management API")
        )
    )]
  struct ApiDoc;

  struct SecurityAddon;

  impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
      if let Some(components) = openapi.components.as_mut() {
        components.add_security_scheme(
          "api_key",
          SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("todo_apikey"))),
        )
      }
    }
  }

  let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
    .nest("/api/v1/todos", todo::router())
    .split_for_parts();

  let router = router
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api.clone()))
    .merge(Redoc::with_url("/redoc", api.clone()))
    // There is no need to create `RapiDoc::with_openapi` because the OpenApi is served
    // via SwaggerUi instead we only make rapidoc to point to the existing doc.
    .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
    // Alternative to above
    // .merge(RapiDoc::with_openapi("/api-docs/openapi2.json", api).path("/rapidoc"))
    .merge(Scalar::with_url("/scalar", api));

  let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
  let listener = TcpListener::bind(&address).await?;
  axum::serve(listener, router.into_make_service()).await
}
