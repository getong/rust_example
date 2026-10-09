use topcoat_gpui_protocol::{CounterAction, DEFAULT_SERVER_URL};

#[tokio::main]
async fn main() -> Result<(), String> {
  let server = std::env::var("TOPCOAT_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.into());
  let before = gpui_topcoat_example::request(&server, None).await?;
  let updated = gpui_topcoat_example::request(&server, Some(CounterAction::Increment)).await?;
  if updated.value != before.value + 1 || updated.revision != before.revision + 1 {
    return Err("Unexpected increment response (use an isolated test server)".into());
  }
  let observed = gpui_topcoat_example::request(&server, None).await?;
  if observed != updated {
    return Err("Read did not observe the updated state".into());
  }
  let reset = gpui_topcoat_example::request(&server, Some(CounterAction::Reset)).await?;
  if reset.value != 0 || reset.revision != updated.revision + 1 {
    return Err("Unexpected reset response".into());
  }
  println!("PASS: desktop HTTP client → Topcoat: read, increment, shared state, reset");
  use gpui_topcoat_example::{DemoCommand, demo_request};
  use topcoat_gpui_protocol::{Profile, TodoCommand};
  let initial = demo_request(&server, DemoCommand::Refresh).await?;
  if std::env::args().any(|arg| arg == "--expect-browser-seed") {
    assert!(
      initial
        .todos
        .iter()
        .any(|todo| todo.title == "Browser task")
    );
    assert_eq!(initial.echoes[0]["source"], "browser");
    assert_eq!(initial.profiles[0].username, "Browser User");
  }
  let created = demo_request(
    &server,
    DemoCommand::Todo(TodoCommand::Create {
      title: "GPUI task".into(),
    }),
  )
  .await?;
  let id = created.todos.last().unwrap().id;
  let changed = demo_request(
    &server,
    DemoCommand::Todo(TodoCommand::SetDone { id, done: true }),
  )
  .await?;
  assert!(
    changed
      .todos
      .iter()
      .find(|todo| todo.id == id)
      .unwrap()
      .done
  );
  let extra = demo_request(
    &server,
    DemoCommand::Todo(TodoCommand::Create {
      title: "Temporary task".into(),
    }),
  )
  .await?;
  let deleted = extra.todos.last().unwrap().id;
  let state = demo_request(
    &server,
    DemoCommand::Todo(TodoCommand::Delete { id: deleted }),
  )
  .await?;
  assert!(!state.todos.iter().any(|todo| todo.id == deleted));
  let value = serde_json::json!({"source":"gpui", "message":"你好", "items":[1,true,null]});
  let suggestions = gpui_topcoat_example::suggest(
    &server,
    topcoat_gpui_protocol::SuggestionKind::Todo,
    "GPUI".into(),
  )
  .await?;
  assert!(
    suggestions
      .items
      .iter()
      .any(|item| item.value == "GPUI task"
        && item.source == topcoat_gpui_protocol::SuggestionSource::Shared)
  );
  let state = demo_request(&server, DemoCommand::Echo(value.clone())).await?;
  assert_eq!(state.echoes[0], value);
  let state = demo_request(
    &server,
    DemoCommand::Profile(Profile {
      username: "GPUI User".into(),
      age: 28,
    }),
  )
  .await?;
  assert_eq!(state.profiles[0].username, "GPUI User");
  assert!(
    demo_request(
      &server,
      DemoCommand::Profile(Profile {
        username: "".into(),
        age: 20
      })
    )
    .await
    .is_err()
  );
  assert!(
    demo_request(
      &server,
      DemoCommand::Todo(TodoCommand::Delete { id: u64::MAX })
    )
    .await
    .is_err()
  );
  println!("PASS: todos CRUD, JSON echo, form submission, browser → desktop state and errors");
  Ok(())
}
