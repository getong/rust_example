use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use takeaway::{Config, Queue, Task, Worker};

enum Job {
    ApiRequest { id: u64 },
    BatchCompute { chunk: u32 },
    Fanout { depth: u8 },
}

impl Task for Job {
    type Priority = NonZeroU32;

    fn priority(&self) -> Self::Priority {
        match self {
            Job::ApiRequest { .. } => nz(100),
            Job::Fanout { .. } => nz(50),
            Job::BatchCompute { .. } => nz(10),
        }
    }
}

fn nz(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("priority must be non-zero")
}

async fn worker(queue: Arc<Queue<Job>>, id: usize) {
    let mut w = Worker::new(queue.as_ref(), id);

    if id == 0 {
        w.enqueue_one(Job::Fanout { depth: 2 });
        w.enqueue_one(Job::ApiRequest { id: 1 });
        w.enqueue_one(Job::BatchCompute { chunk: 8 });
    }

    while let Some(job) = w.next().await {
        match job {
            Job::ApiRequest { id } => {
                let _ = id;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Job::BatchCompute { chunk } => {
                let _ = tokio::task::spawn_blocking(move || chunk as u64 * 1024).await;
            }
            Job::Fanout { depth } => {
                if depth > 0 {
                    w.enqueue_one(Job::ApiRequest { id: depth as u64 * 10 });
                    w.enqueue_one(Job::BatchCompute { chunk: depth as u32 * 4 });
                    w.enqueue_one(Job::Fanout { depth: depth - 1 });
                }
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let queue = Arc::new(Config::default().with_oneshot(true).build());
    let num_workers = queue.config().num_workers().get();

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let mut handles = Vec::with_capacity(num_workers);
            for id in 0..num_workers {
                let queue = queue.clone();
                handles.push(tokio::task::spawn_local(async move {
                    worker(queue, id).await;
                }));
            }

            for handle in handles {
                let _ = handle.await;
            }
        })
        .await;
}
