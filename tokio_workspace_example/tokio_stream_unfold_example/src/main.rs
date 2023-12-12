use futures::{future, future::BoxFuture, stream, FutureExt, StreamExt}; // 0.3.13
use std::time::{Duration, Instant};
use tokio::time; // 1.3.0

#[tokio::main]
async fn main() {
  let now = Instant::now();
  let forever = stream::unfold((), |()| async {
    eprintln!("Loop starting at {:?}", Instant::now());

    // Resolves when all pages are done
    let batch_of_pages = future::join_all(all_pages());

    // Resolves when both all pages and a delay of 1 second is done
    future::join(batch_of_pages, time::sleep(Duration::from_secs(1))).await;

    Some(((), ()))
  });

  forever.take(5).for_each(|_| async {}).await;
  eprintln!("Took {:?}", now.elapsed());

  // another example
  let stream = stream::unfold(0, |state| async move {
    if state <= 2 {
      let next_state = state + 1;
      let yielded = state * 2;
      Some((yielded, next_state))
    } else {
      None
    }
  });

  let result = stream.collect::<Vec<i32>>().await;
  assert_eq!(result, vec![0, 2, 4]);
}

fn all_pages() -> Vec<BoxFuture<'static, ()>> {
  vec![page("a", 100).boxed(), page("b", 200).boxed()]
}

async fn page(name: &'static str, time_ms: u64) {
  eprintln!("page {} starting", name);
  time::sleep(Duration::from_millis(time_ms)).await;
  eprintln!("page {} done", name);
}
