mod multi_raft_kv;
mod octopii_task_queue;

#[tokio::main]
async fn main() -> octopii::Result<()> {
  println!("=== octopii demo: uses octopii's vendored openraft ===");
  octopii_task_queue::run().await?;

  println!("\n=== direct openraft demo: uses openraft + openraft-multi from this crate ===");
  multi_raft_kv::run_demo().await?;

  Ok(())
}
