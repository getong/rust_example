//! Adapted from the Axum workspace's TODO, JSON echo and Form examples.
use tokio::sync::Mutex;
use topcoat::{
  Result,
  context::{Cx, app_context},
  router::{
    RouterBuilder,
    content::{Form, Json},
    error::{bad_request, not_found},
    page, route,
  },
  view::{View, view},
};
use topcoat_gpui_protocol::{DemoSnapshot, Profile, Todo, TodoCommand};

#[derive(Default)]
struct Store {
  snapshot: DemoSnapshot,
  next_id: u64,
}
#[derive(Default)]
struct Demos(Mutex<Store>);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
  builder
    .app_context(Demos::default())
    .page(todos_page)
    .page(echo_page)
    .page(profile_page)
    .route(read)
    .route(todos)
    .route(echo)
    .route(profile)
}

#[route(GET "/api/demos")]
async fn read(cx: &Cx) -> Result<Json<DemoSnapshot>> {
  Ok(Json(
    app_context::<Demos>(cx).0.lock().await.snapshot.clone(),
  ))
}

#[route(POST "/api/todos")]
async fn todos(cx: &Cx, Json(command): Json<TodoCommand>) -> Result<Json<DemoSnapshot>> {
  let mut store = app_context::<Demos>(cx).0.lock().await;
  match command {
    TodoCommand::Create { title } => {
      let title = title.trim();
      if title.is_empty() || title.chars().count() > 120 {
        return Err(bad_request("Title must contain 1–120 characters").into());
      }
      if store.snapshot.todos.len() >= 200 {
        return Err(bad_request("At most 200 todos").into());
      }
      store.next_id += 1;
      let id = store.next_id;
      store.snapshot.todos.push(Todo {
        id,
        title: title.into(),
        done: false,
      });
    }
    TodoCommand::SetDone { id, done } => {
      let todo = store
        .snapshot
        .todos
        .iter_mut()
        .find(|todo| todo.id == id)
        .ok_or_else(|| not_found())?;
      todo.done = done;
    }
    TodoCommand::Delete { id } => {
      let index = store
        .snapshot
        .todos
        .iter()
        .position(|todo| todo.id == id)
        .ok_or_else(|| not_found())?;
      store.snapshot.todos.remove(index);
    }
  }
  Ok(Json(store.snapshot.clone()))
}

#[route(POST "/api/echo")]
async fn echo(cx: &Cx, Json(value): Json<serde_json::Value>) -> Result<Json<DemoSnapshot>> {
  if value.to_string().len() > 65536 {
    return Err(bad_request("JSON must be at most 64 KiB").into());
  }
  let mut store = app_context::<Demos>(cx).0.lock().await;
  store.snapshot.echoes.insert(0, value);
  store.snapshot.echoes.truncate(20);
  Ok(Json(store.snapshot.clone()))
}

#[route(POST "/api/profile")]
async fn profile(cx: &Cx, Form(mut input): Form<Profile>) -> Result<Json<DemoSnapshot>> {
  input.username = input.username.trim().to_owned();
  if input.username.is_empty() || input.username.chars().count() > 80 || input.age > 150 {
    return Err(bad_request("Username must contain 1–80 characters; age must be 0–150").into());
  }
  let mut store = app_context::<Demos>(cx).0.lock().await;
  store.snapshot.profiles.insert(0, input);
  store.snapshot.profiles.truncate(20);
  Ok(Json(store.snapshot.clone()))
}

#[page("/todos")]
async fn todos_page(cx: &Cx) -> Result<impl View> {
  Ok(shell(cx, "todos"))
}
#[page("/echo")]
async fn echo_page(cx: &Cx) -> Result<impl View> {
  Ok(shell(cx, "echo"))
}
#[page("/profile")]
async fn profile_page(cx: &Cx) -> Result<impl View> {
  Ok(shell(cx, "profile"))
}

fn shell<'a>(cx: &'a Cx, kind: &'static str) -> impl View + 'a {
  view! { cx =>
      <!DOCTYPE html>
      <html lang="zh-CN"><head>
          <meta charset="utf-8">
          <meta name="viewport" content="width=device-width, initial-scale=1">
          <title>"Topcoat × GPUI-kit 协作示例"</title>
          <script src="/demos.js" defer=(true)></script>
      </head>
      <body data-page=(kind) style="max-width:900px;margin:48px auto;padding:24px;font:18px system-ui;background:#f4f6fb;color:#182030">
          <h1>"Topcoat × GPUI-kit"</h1>
          <nav><a href="/">"计数器"</a>" · "<a href="/todos">"待办事项"</a>" · "<a href="/echo">"JSON 回显"</a>" · "<a href="/profile">"表单提交"</a>" · "<a href="/studio">"配色实验室"</a></nav>
          <p>"与桌面 Tab 共享数据，每两秒同步；服务重启后清空。"</p>
          if kind == "todos" {
              <h2>"待办事项"</h2>
              <form id="editor"><input id="title" placeholder="输入待办事项" maxlength="120" required=(true)><button type="submit">"添加"</button></form>
          } else if kind == "echo" {
              <h2>"JSON 回显与共享历史"</h2>
              <form id="editor"><textarea id="json" rows="6" cols="60">"{\"message\":\"Hello from browser\"}"</textarea><br><button type="submit">"发送 JSON"</button></form>
          } else {
              <h2>"表单提交与共享历史"</h2>
              <form id="editor"><label>"姓名 "<input id="username" maxlength="80" required=(true)></label><label>" 年龄 "<input id="age" type="number" min="0" max="150" value="18" required=(true)></label><button type="submit">"提交表单"</button></form>
          }
          <button id="refresh" type="button">"刷新"</button>
          <p id="status" role="status">"正在连接…"</p>
          <div id="results"></div>
      </body></html>
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use topcoat::router::{Body, Router, request::Request, to_bytes};

  async fn call(router: &Router, path: &str, body: Option<&str>, form: bool) -> (u16, Vec<u8>) {
    let response = router
      .handle(
        Request::builder()
          .method(if body.is_some() { "POST" } else { "GET" })
          .uri(path)
          .header(
            "content-type",
            if form {
              "application/x-www-form-urlencoded"
            } else {
              "application/json"
            },
          )
          .body(Body::from(body.unwrap_or("").to_owned()))
          .unwrap(),
      )
      .await;
    (
      response.status().as_u16(),
      to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec(),
    )
  }

  #[tokio::test]
  async fn todo_crud_and_rejected_writes() {
    let router = crate::router();
    let (status, bytes) = call(
      &router,
      "/api/todos",
      Some(r#"{"action":"create","title":"  Shared task  "}"#),
      false,
    )
    .await;
    assert_eq!(status, 200);
    let state: DemoSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state.todos[0].title, "Shared task");
    let id = state.todos[0].id;
    let command = format!(r#"{{"action":"set_done","id":{id},"done":true}}"#);
    assert_eq!(
      call(&router, "/api/todos", Some(&command), false).await.0,
      200
    );
    assert_eq!(
      call(
        &router,
        "/api/todos",
        Some(r#"{"action":"create","title":" "}"#),
        false
      )
      .await
      .0,
      400
    );
    assert_eq!(
      call(
        &router,
        "/api/todos",
        Some(r#"{"action":"delete","id":999}"#),
        false
      )
      .await
      .0,
      404
    );
    let (_, bytes) = call(&router, "/api/demos", None, false).await;
    let state: DemoSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state.todos.len(), 1);
    assert!(state.todos[0].done);
    let command = format!(r#"{{"action":"delete","id":{id}}}"#);
    let (_, bytes) = call(&router, "/api/todos", Some(&command), false).await;
    assert!(
      serde_json::from_slice::<DemoSnapshot>(&bytes)
        .unwrap()
        .todos
        .is_empty()
    );
  }

  #[tokio::test]
  async fn echo_and_form_are_shared_validated_and_bounded() {
    let router = crate::router();
    for n in 0..25 {
      assert_eq!(
        call(
          &router,
          "/api/echo",
          Some(&format!(r#"{{"index":{n}}}"#)),
          false
        )
        .await
        .0,
        200
      );
      assert_eq!(
        call(
          &router,
          "/api/profile",
          Some(&format!("username=Person{n}&age=20")),
          true
        )
        .await
        .0,
        200
      );
    }
    assert_eq!(
      call(&router, "/api/echo", Some("bad json"), false).await.0,
      400
    );
    assert_eq!(
      call(&router, "/api/profile", Some("username=&age=20"), true)
        .await
        .0,
      400
    );
    assert_eq!(
      call(&router, "/api/profile", Some("username=Test&age=151"), true)
        .await
        .0,
      400
    );
    let (_, bytes) = call(&router, "/api/demos", None, false).await;
    let state: DemoSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state.echoes.len(), 20);
    assert_eq!(state.echoes[0]["index"], 24);
    assert_eq!(state.profiles.len(), 20);
    assert_eq!(state.profiles[0].username, "Person24");
    for path in ["/", "/todos", "/echo", "/profile"] {
      let (status, bytes) = call(&router, path, None, false).await;
      assert_eq!(status, 200);
      assert!(String::from_utf8(bytes).unwrap().contains("/todos"));
    }
  }
}
