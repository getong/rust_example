use tokio::sync::Mutex;
use topcoat::{
  Result,
  context::{Cx, app_context},
  router::{
    RouterBuilder,
    content::{Js, Json},
    error::bad_request,
    page, route,
  },
  view::{View, view},
};
use topcoat_gpui_protocol::{StudioCommand, StudioSnapshot};

#[derive(Default)]
struct Studio(Mutex<StudioSnapshot>);
pub fn register(builder: RouterBuilder) -> RouterBuilder {
  builder
    .app_context(Studio::default())
    .page(studio_page)
    .route(read_studio)
    .route(apply_studio)
    .route(studio_script)
}
#[route(GET "/api/studio")]
async fn read_studio(cx: &Cx) -> Result<Json<StudioSnapshot>> {
  Ok(Json(app_context::<Studio>(cx).0.lock().await.clone()))
}
#[route(POST "/api/studio")]
async fn apply_studio(cx: &Cx, Json(command): Json<StudioCommand>) -> Result<Json<StudioSnapshot>> {
  let StudioCommand::Apply { theme, intensity } = command;
  if intensity > 100 {
    return Err(bad_request("Intensity must be 0–100").into());
  }
  let mut state = app_context::<Studio>(cx).0.lock().await;
  let revision = state
    .revision
    .checked_add(1)
    .ok_or_else(|| bad_request("Revision overflow"))?;
  *state = StudioSnapshot {
    theme,
    intensity,
    revision,
  };
  Ok(Json(state.clone()))
}
#[route(GET "/assets/studio")]
async fn studio_script() -> Result<Js<&'static str>> {
  Ok(Js(include_str!(concat!(env!("OUT_DIR"), "/studio.js"))))
}
#[page("/studio")]
async fn studio_page() -> Result<impl View> {
  Ok(view! {
    <!DOCTYPE html>
    <html lang="zh-CN"><head>
      <meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
      <title>"配色实验室 · Topcoat × GPUI"</title>
      <script src="/assets/studio" defer=(true)></script>
    </head>
    <body style="max-width:800px;margin:48px auto;padding:24px;font:18px system-ui;background:#f4f6fb;color:#182030">
      <nav><a href="/">"计数器"</a>" · "<a href="/todos">"待办"</a>" · "<a href="/echo">"JSON"</a>" · "<a href="/profile">"表单"</a>" · "<a href="/studio">"配色实验室"</a></nav>
      <h1>"配色实验室"</h1>
      <p>"调整配色立即预览，发布后同步到桌面「配色实验室」。桌面发布的修改也会在这里显示。"</p>
      <label>"主题 "<select id="theme"><option value="ocean">"海洋蓝"</option><option value="sunset">"落日橙"</option><option value="forest">"森林绿"</option></select></label>
      <label>" 强度 "<input id="intensity" type="range" min="0" max="100" step="1" value="70"></label>
      <p id="preview-label"></p>
      <div id="preview" style="height:180px;border-radius:20px;transition:background-color .15s,opacity .15s" aria-label="配色预览"></div>
      <p id="draft-status"></p>
      <button id="publish" type="button">"发布到两端"</button>" "<button id="refresh" type="button">"使用最新发布"</button>
      <h2>"两端共享配色"</h2><p id="published" aria-live="polite">"正在读取…"</p>
      <p id="status" role="status">"正在连接…"</p>
    </body></html>
  })
}

#[cfg(test)]
mod tests {
  use topcoat::router::{Body, request::Request, to_bytes};
  #[tokio::test]
  async fn studio_has_independent_state_and_rejects_invalid_commands() {
    let router = crate::router();
    for (body, status) in [
      (r#"{"action":"apply","theme":"sunset","intensity":42}"#, 200),
      (
        r#"{"action":"apply","theme":"unknown","intensity":42}"#,
        400,
      ),
      (r#"{"action":"apply","theme":"ocean","intensity":101}"#, 400),
      (r#"{"action":"apply","theme":"ocean","intensity":1.5}"#, 400),
      (
        r#"{"action":"apply","theme":"ocean","intensity":50,"extra":true}"#,
        400,
      ),
    ] {
      let response = router
        .handle(
          Request::builder()
            .method("POST")
            .uri("/api/studio")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap(),
        )
        .await;
      assert_eq!(response.status().as_u16(), status);
    }
    for (path, expected) in [
      (
        "/api/studio",
        serde_json::json!({"theme":"sunset","intensity":42,"revision":1}),
      ),
      ("/api/counter", serde_json::json!({"value":0,"revision":0})),
      (
        "/api/demos",
        serde_json::json!({"todos":[],"echoes":[],"profiles":[]}),
      ),
    ] {
      let response = router
        .handle(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await;
      let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
      assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        expected
      );
    }
  }
}
