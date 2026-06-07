use std::time::Duration;

use apalis::prelude::*;
use apalis_file_storage::JsonStorage;
use apalis_workflow::*;

#[tokio::main]
async fn main() {
  let workflow = Workflow::new("odd-numbers-workflow")
    .delay_for(Duration::from_millis(1000))
    .and_then(|a: usize| async move { Ok::<_, BoxDynError>((0 .. a).collect::<Vec<_>>()) })
    .filter_map(|x| async move { if x % 2 != 0 { Some(x) } else { None } })
    .and_then(|a: Vec<usize>| async move {
      println!("Sum: {}", a.iter().sum::<usize>());
      Ok::<_, BoxDynError>(())
    });

  let mut in_memory = JsonStorage::new_temp().unwrap();

  in_memory.push_start(10).await.unwrap();

  let worker = WorkerBuilder::new("rango-tango")
    .backend(in_memory)
    .on_event(|_ctx, ev| {
      println!("On Event = {:?}", ev);
    })
    .build(workflow);
  worker.run().await.unwrap();
}
