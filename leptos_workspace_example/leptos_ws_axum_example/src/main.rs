#[cfg(feature = "ssr")]
pub mod fileserv;
pub mod messages;
#[cfg(feature = "ssr")]
use axum::response::Response as AxumResponse;
#[cfg(feature = "ssr")]
use axum::{
  Router,
  routing::{get, post},
};
#[cfg(feature = "ssr")]
use axum::{
  extract::{FromRef, Path, Request, State},
  response::IntoResponse,
};
#[cfg(feature = "ssr")]
use config::get_configuration;
#[cfg(feature = "ssr")]
use http::HeaderMap;
#[cfg(feature = "ssr")]
use leptos::*;
#[cfg(feature = "ssr")]
use leptos::{
  config::LeptosOptions,
  prelude::{provide_context, *},
};
#[cfg(feature = "ssr")]
use leptos_axum::{AxumRouteListing, handle_server_fns_with_context};
#[cfg(feature = "ssr")]
use leptos_axum::{LeptosRoutes, generate_route_list_with_exclusions_and_ssg_and_context};
#[cfg(feature = "ssr")]
use leptos_ws::server_signals::ServerSignals;
#[cfg(feature = "ssr")]
use leptos_ws_axum_example::app::*;

#[cfg(feature = "ssr")]
use crate::fileserv::file_and_error_handler;

#[cfg(feature = "ssr")]
#[derive(Clone, FromRef)]
pub struct AppState {
  server_signals: ServerSignals,
  routes: Option<Vec<AxumRouteListing>>,
  options: LeptosOptions,
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
  use std::{net::SocketAddr, sync::Arc, time::Duration};

  use tokio::time::sleep;
  use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

  pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <link rel="stylesheet" href="pkg/axum_example.css"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options=options islands=true/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
  }

  async fn leptos_routes_handler(state: State<AppState>, req: Request) -> AxumResponse {
    let state1 = state.0.clone();
    let options2 = state.clone().0.options.clone();
    let handler = leptos_axum::render_route_with_context(
      state.routes.clone().unwrap(),
      move || {
        provide_context(state1.options.clone());
        provide_context(state1.server_signals.clone());
      },
      move || shell(options2.clone()),
    );
    handler(state, req).await.into_response()
  }
  async fn server_fn_handler(
    State(state): State<AppState>,
    _path: Path<String>,
    _headers: HeaderMap,
    _query: axum::extract::RawQuery,
    request: Request,
  ) -> impl IntoResponse {
    handle_server_fns_with_context(
      move || {
        provide_context(state.options.clone());
        provide_context(state.server_signals.clone());
      },
      request,
    )
    .await
  }
  let governor_conf = Arc::new(
    GovernorConfigBuilder::default()
      .per_second(2)
      .burst_size(5)
      .finish()
      .unwrap(),
  );

  let governor_limiter = governor_conf.limiter().clone();
  let interval = Duration::from_secs(60);
  // a separate background task to clean up
  tokio::spawn(async move {
    loop {
      sleep(interval).await;
      governor_limiter.retain_recent();
    }
  });
  simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");
  let server_signals = ServerSignals::new();
  // let signal = ServerSignal::new("counter".to_string(), 1);
  // build our application with a route
  let conf = get_configuration(None).unwrap();
  let leptos_options = conf.leptos_options;
  let mut state = AppState {
    options: leptos_options.clone(),
    routes: None,
    server_signals: server_signals.clone(),
  };
  // Setting get_configuration(None) means we'll be using cargo-leptos's env values
  // For deployment these variables are:
  // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
  // Alternately a file can be specified such as Some("Cargo.toml")
  // The file would need to be included with the executable when moved to deployment
  let addr = leptos_options.site_addr;
  let state2 = state.clone();
  let (routes, _) = generate_route_list_with_exclusions_and_ssg_and_context(
    || view! { <App/> },
    None,
    move || provide_context(state2.server_signals.clone()),
  );
  state.routes = Some(routes.clone());
  let app = Router::new()
    .route("/api/{*fn_name}", post(server_fn_handler))
    .layer(GovernorLayer {
      config: governor_conf,
    })
    .route(
      "/ws",
      get(leptos_ws::axum::websocket(state.server_signals.clone())),
    )
    .leptos_routes_with_handler(routes, get(leptos_routes_handler))
    .fallback(file_and_error_handler)
    .with_state(state);
  // run our app with hyper
  // `axum::Server` is a re-export of `hyper::Server`
  leptos::logging::log!("listening on http://{}", &addr);
  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  axum::serve(
    listener,
    app.into_make_service_with_connect_info::<SocketAddr>(),
  )
  .await
  .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
  // no client-side main function
  // unless we want this to work with e.g., Trunk for a purely client-side app
  // see lib.rs for hydration function instead
}
