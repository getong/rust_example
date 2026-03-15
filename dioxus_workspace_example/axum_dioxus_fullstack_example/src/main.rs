#![allow(non_snake_case)]
#[cfg(feature = "server")]
use std::{io, net::SocketAddr, path::PathBuf};

use dioxus::prelude::*;

// The entry point for the server
#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
  // Get the address the server should run on. If the CLI is running, the CLI proxies fullstack into
  // the main address and we use the generated address the CLI gives us
  let address = dioxus::cli_config::fullstack_address_or_localhost();

  // Set up the axum router
  let serve_config = ServeConfig::new();
  let router = if resolved_public_path().is_some_and(|path| path.is_dir()) {
    axum::Router::new()
      // Serve the fullstack app when dx-generated static assets are available.
      .serve_dioxus_application(serve_config, App)
  } else {
    eprintln!(
      "No generated public directory found; serving SSR + server functions only. Use `dx serve` \
       for hydrated web assets."
    );
    axum::Router::new().serve_api_application(serve_config, App)
  };

  // Finally, we can launch the server
  let router = router.into_make_service();
  let listener = bind_listener(address).await.unwrap_or_else(|error| {
    panic!("failed to bind server listener on {address}: {error}");
  });
  eprintln!("Listening on http://{}", listener.local_addr().unwrap());
  axum::serve(listener, router).await.unwrap();
}

// For any other platform, we just launch the app
#[cfg(not(feature = "server"))]
fn main() {
  dioxus::launch(App);
}

#[cfg(feature = "server")]
fn resolved_public_path() -> Option<PathBuf> {
  std::env::var("DIOXUS_PUBLIC_PATH")
    .ok()
    .map(PathBuf::from)
    .or_else(|| {
      std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("public")))
    })
}

#[cfg(feature = "server")]
async fn bind_listener(address: SocketAddr) -> io::Result<tokio::net::TcpListener> {
  match tokio::net::TcpListener::bind(address).await {
    Ok(listener) => Ok(listener),
    Err(error) if error.kind() == io::ErrorKind::AddrInUse && !has_explicit_bind_address() => {
      eprintln!(
        "Address {address} is already in use; falling back to an available port for plain `cargo \
         run`."
      );
      tokio::net::TcpListener::bind(SocketAddr::new(address.ip(), 0)).await
    }
    Err(error) => Err(error),
  }
}

#[cfg(feature = "server")]
fn has_explicit_bind_address() -> bool {
  std::env::var_os("IP").is_some() || std::env::var_os("PORT").is_some()
}

#[component]
fn App() -> Element {
  let mut meaning = use_signal(|| None);

  rsx! {
      h1 { "Meaning of life: {meaning:?}" }
      button {
          onclick: move |_| async move {
              if let Ok(data) = get_meaning("life the universe and everything".into()).await {
                  meaning.set(data);
              }
          },
          "Run a server function"
      }
  }
}

#[server]
async fn get_meaning(of: String) -> Result<Option<u32>, ServerFnError> {
  Ok(of.contains("life").then(|| 42))
}

// copy from https://docs.rs/dioxus-fullstack/0.6.3/dioxus_fullstack/#axum-integration
// bug , see https://github.com/DioxusLabs/dioxus/issues/3790
// also see https://github.com/DioxusLabs/dioxus/pull/3693
