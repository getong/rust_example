use tokio::task;

#[tokio::main]
async fn main() {
  match task::try_id() {
    Some(task_id) => println!("Current task ID: {:?}", task_id),
    None => println!("Can't get a task id when not inside a task"),
  }

  tokio::spawn(async {
    let task_id = task::id();
    println!("Spawned task ID: {:?}", task_id);
  })
  .await
  .unwrap();
}
