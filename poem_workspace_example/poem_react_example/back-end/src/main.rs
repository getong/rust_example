use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route};
use poem_openapi::{OpenApi, OpenApiService, Webhook};

mod api;
use api::make_service;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
  let mut api_service = make_service();
  let mut app = Route::new();

  (api_service, app) = add_swagger_ui(api_service, app);

  app = app.nest("/", api_service);

  poem::Server::new(TcpListener::bind("0.0.0.0:3001"))
    .run(app.with(Cors::new().allow_origin("http://localhost:3000")))
    .await
}

#[cfg(debug_assertions)]
fn add_swagger_ui<T: OpenApi + 'static, W: Webhook + 'static>(
  mut api_service: OpenApiService<T, W>,
  mut app: Route,
) -> (OpenApiService<T, W>, Route) {
  api_service = api_service.server("http://localhost:3001");
  let ui = api_service.swagger_ui();
  app = app.nest("/docs", ui);
  (api_service, app)
}

#[cfg(not(debug_assertions))]
fn add_swagger_ui<T: OpenApi + 'static, W: Webhook + 'static>(
  api_service: OpenApiService<T, W>,
  app: Route,
) -> (OpenApiService<T, W>, Route) {
  (api_service, app)
}
