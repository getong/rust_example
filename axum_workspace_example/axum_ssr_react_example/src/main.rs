use std::{cell::RefCell, fs::read_to_string, time::Instant};

use axum::{Router, response::Html, routing::get};
use ssr_rs::Ssr;

thread_local! {
    static SSR: RefCell<Ssr<'static, 'static>> = RefCell::new({
        let js_code = read_to_string("client/dist/ssr/index.js").unwrap();
        let polyfill = r#"
if (typeof MessageChannel === 'undefined') {
    globalThis.MessageChannel = function() {
        const channel = {};
        channel.port1 = { postMessage: function() {}, onmessage: null };
        channel.port2 = { postMessage: function() {}, onmessage: null };
        return channel;
    };
}
"#;
        let enhanced_js = format!("{}\n{}", polyfill, js_code);
        Ssr::from(enhanced_js, "SSR").unwrap()
    })
}

#[tokio::main]
async fn main() {
  Ssr::create_platform();

  // build our application with a single route
  let app = Router::new().route("/", get(root));

  // run our app with hyper, listening globally on port 3000
  let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
  println!("Server running on http://0.0.0.0:8080");
  axum::serve(listener, app).await.unwrap();
}

async fn root() -> Html<String> {
  let start = Instant::now();
  let result = SSR.with(|ssr| ssr.borrow_mut().render_to_string(Some("Index")));
  println!("Elapsed: {:?}", start.elapsed());

  match result {
    Ok(html) => {
      let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>React SSR</title>
</head>
<body>
    <div id="root">{}</div>
</body>
</html>"#,
        html
      );
      Html(full_html)
    }
    Err(e) => {
      eprintln!("SSR Error: {}", e);
      Html("<html><body><h1>SSR Error</h1><p>Failed to render page</p></body></html>".to_string())
    }
  }
}
