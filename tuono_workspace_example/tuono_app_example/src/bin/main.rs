use tuono_lib::{
  Mode, Server,
  axum::{Router, routing::get},
  tokio, tuono_internal_init_v8_platform,
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
  println!("\n  ⚡ Tuono v0.19.2");

  let router = Router::new()
    // ROUTE_BUILDER
    .route("/api/mod", get(api_mod::get_tuono_internal_api))
    .route("/", get(index::tuono_internal_route))
    .route("/__tuono/data/", get(index::tuono_internal_api));

  Server::init(router, MODE).await.start().await
}
