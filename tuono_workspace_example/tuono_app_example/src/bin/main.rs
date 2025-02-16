use tuono_lib::{
  axum::{routing::get, Router},
  tokio, tuono_internal_init_v8_platform, Mode, Server,
};

const MODE: Mode = Mode::Prod;

// MODULE_IMPORTS
use tuono_app_example::routes::{api as api_mod, index};
// #[path = "../routes/api/mod.rs"]
// mod api_mod;
// #[path = "../routes/index.rs"]
// mod index;

#[tokio::main]
async fn main() {
  tuono_internal_init_v8_platform();
  println!("\n  ⚡ Tuono v0.17.8");

  let router = Router::new()
    // ROUTE_BUILDER
    .route("/api/mod", get(api_mod::get__tuono_internal_api))
    .route("/", get(index::tuono__internal__route))
    .route("/__tuono/data/", get(index::tuono__internal__api));

  Server::init(router, MODE).await.start().await
}
