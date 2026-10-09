mod demos;
mod secure;
mod studio;

use tokio::sync::Mutex;
use topcoat::{
  Result,
  context::{Cx, app_context},
  router::{
    Router,
    content::{Js, Json},
    page, route,
  },
  view::{View, view},
};
use topcoat_gpui_protocol::{CounterAction, CounterSnapshot, UpdateCounter};

#[derive(Default)]
struct Counter(Mutex<CounterSnapshot>);

fn router() -> Router {
  studio::register(demos::register(Router::builder()))
    .app_context(Counter::default())
    .page(home)
    .route(script)
    .route(demos_script)
    .route(read_counter)
    .route(update_counter)
    .build()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
  eprintln!(
    "Topcoat × GPUI-kit: http://{}:{}",
    std::env::var("HOST").unwrap_or("127.0.0.1".into()),
    std::env::var("PORT").unwrap_or("3000".into())
  );
  topcoat::start(secure::wrap(router())).await
}

#[route(GET "/api/counter")]
async fn read_counter(cx: &Cx) -> Result<Json<CounterSnapshot>> {
  Ok(Json(app_context::<Counter>(cx).0.lock().await.clone()))
}

#[route(POST "/api/counter")]
async fn update_counter(
  cx: &Cx,
  Json(input): Json<UpdateCounter>,
) -> Result<Json<CounterSnapshot>> {
  let mut snapshot = app_context::<Counter>(cx).0.lock().await;
  let value = match input.action {
    CounterAction::Increment => snapshot.value.checked_add(1),
    CounterAction::Reset => Some(0),
  }
  .ok_or_else(|| topcoat::router::error::bad_request("Counter overflow"))?;
  let revision = snapshot
    .revision
    .checked_add(1)
    .ok_or_else(|| topcoat::router::error::bad_request("Revision overflow"))?;
  *snapshot = CounterSnapshot { value, revision };
  Ok(Json(snapshot.clone()))
}

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
  let _ = cx;
  Ok(view! {
      <!DOCTYPE html>
      <html lang="zh-CN">
          <head>
              <meta charset="utf-8">
              <meta name="viewport" content="width=device-width, initial-scale=1">
              <title>"Topcoat × GPUI-kit"</title>
              <script src="/app.js" defer=(true)></script>
          </head>
          <body style="max-width:720px;margin:64px auto;padding:24px;font:20px system-ui;background:#f4f6fb;color:#182030">
              <h1>"Topcoat × GPUI-kit"</h1>
              <nav><a href="/">"计数器"</a>" · "<a href="/todos">"待办事项"</a>" · "<a href="/echo">"JSON 回显"</a>" · "<a href="/profile">"表单提交"</a>" · "<a href="/studio">"配色实验室"</a></nav>
              <p>"网页与桌面共享同一个计数器，每两秒同步一次。"</p>
              <h2>"计数："<span id="value">"…"</span></h2>
              <p>"版本："<span id="revision">"…"</span></p>
              <button id="increment" type="button">"+1"</button>
              <button id="reset" type="button">"重置"</button>
              <button id="refresh" type="button">"刷新"</button>
              <p id="status" role="status">"正在连接…"</p>
              <p>"状态保存在服务端内存中，重启服务后归零。"</p>
          </body>
      </html>
  })
}

#[route(GET "/app.js")]
async fn script() -> Result<Js<&'static str>> {
  Ok(Js(include_str!(concat!(env!("OUT_DIR"), "/app.js"))))
}

#[cfg(test)]
mod tests {
  use topcoat::router::{Body, request::Request, to_bytes};

  use super::*;

  async fn send(router: &Router, method: &str, body: &str) -> (u16, Vec<u8>) {
    let response = router
      .handle(
        Request::builder()
          .method(method)
          .uri("/api/counter")
          .header("content-type", "application/json")
          .body(Body::from(body.to_owned()))
          .unwrap(),
      )
      .await;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, bytes.to_vec())
  }

  #[tokio::test]
  async fn shared_state_and_invalid_input() {
    let router = router();
    for (method, body, value, revision) in [
      ("GET", "", 0, 0),
      ("POST", r#"{"action":"increment"}"#, 1, 1),
      ("GET", "", 1, 1),
      ("POST", r#"{"action":"reset"}"#, 0, 2),
    ] {
      let (status, bytes) = send(&router, method, body).await;
      assert_eq!(status, 200);
      assert_eq!(
        serde_json::from_slice::<CounterSnapshot>(&bytes).unwrap(),
        CounterSnapshot { value, revision }
      );
    }
    assert_eq!(send(&router, "POST", r#"{"action":"bad"}"#).await.0, 400);
    assert_eq!(send(&router, "POST", "not json").await.0, 400);
    let (_, bytes) = send(&router, "GET", "").await;
    assert_eq!(
      serde_json::from_slice::<CounterSnapshot>(&bytes)
        .unwrap()
        .revision,
      2
    );
  }

  #[tokio::test]
  async fn compiled_browser_scripts_are_served() {
    let router = router();
    for (path, endpoint) in [
      ("/app.js", "/api/counter"),
      ("/demos.js", "/api/demos"),
      ("/assets/studio", "/api/studio"),
    ] {
      let response = router
        .handle(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await;
      assert_eq!(response.status().as_u16(), 200);
      assert!(
        response.headers()["content-type"]
          .to_str()
          .unwrap()
          .contains("javascript")
      );
      let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
      let bundle = std::str::from_utf8(&bytes).unwrap();
      assert!(bundle.contains(endpoint));
      assert!(bundle.contains("Generated from TypeScript"));
    }
  }

  #[tokio::test]
  async fn concurrent_updates_are_not_lost() {
    let router = std::sync::Arc::new(router());
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0 .. 32 {
      let router = router.clone();
      tasks.spawn(async move {
        assert_eq!(
          send(&router, "POST", r#"{"action":"increment"}"#).await.0,
          200
        );
      });
    }
    while let Some(result) = tasks.join_next().await {
      result.unwrap();
    }
    let (_, bytes) = send(&router, "GET", "").await;
    assert_eq!(
      serde_json::from_slice::<CounterSnapshot>(&bytes).unwrap(),
      CounterSnapshot {
        value: 32,
        revision: 32
      }
    );
  }
}

#[route(GET "/demos.js")]
async fn demos_script() -> Result<Js<&'static str>> {
  Ok(Js(include_str!(concat!(env!("OUT_DIR"), "/demos.js"))))
}
