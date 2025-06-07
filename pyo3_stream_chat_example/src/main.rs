use pyo3::{prelude::*, types::IntoPyDict};
use tokio::task;

async fn create_token_for_user(user_id: &str) -> PyResult<String> {
  let user_id = user_id.to_string();
  task::spawn_blocking(move || {
    Python::with_gil(|py| {
      // Import the stream_chat module
      let stream_chat = py.import("stream_chat")?;
      // Create StreamChat client
      let kwargs = [
        ("api_key", "{{ api_key }}"),
        ("api_secret", "{{ api_secret }}"),
      ]
      .into_py_dict(py)?;
      let server_client = stream_chat.getattr("StreamChat")?.call((), Some(&kwargs))?;
      // Create token for the user
      let token = server_client.call_method1("create_token", (&user_id,))?;
      let token_str: String = token.extract()?;
      Ok(token_str)
    })
  })
  .await
  .unwrap()
}

#[tokio::main]
async fn main() -> PyResult<()> {
  let users = vec!["john", "alice", "bob", "charlie"];
  let mut handles = Vec::new();

  for user in users {
    let handle = tokio::spawn(async move {
      match create_token_for_user(user).await {
        Ok(token) => println!("Token for {}: {}", user, token),
        Err(e) => eprintln!("Error creating token for {}: {}", user, e),
      }
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.await.unwrap();
  }
  Ok(())
}
