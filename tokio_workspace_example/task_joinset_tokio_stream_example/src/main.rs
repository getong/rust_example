use async_stream::stream;
use chrono::prelude::Local;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tokio_stream::{Stream, StreamExt};

#[allow(dead_code)]
#[derive(Debug)]
struct SomeError {
    msg: String,
}

fn hms() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

async fn operate(time: u64, result: Option<i64>) -> Result<i64, SomeError> {
    match result {
        Some(x) => {
            println!("{}: Spending {time} seconds doing operations ...", hms());
            sleep(Duration::from_secs(time)).await;
            println!("{}: Operations done after {time} seconds!", hms());
            Ok(x)
        }
        None => {
            println!("{}: Spending {time} seconds doing risky things ...", hms());
            sleep(Duration::from_secs(time)).await;
            Err(SomeError {
                msg: format!("Operation failed after {time} seconds!"),
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), SomeError> {
    let mut tasks = JoinSet::new();
    tasks.spawn(operate(5, Some(42)));
    tasks.spawn(operate(1, Some(17)));
    tasks.spawn(operate(3, None));
    tasks.spawn(operate(2, Some(23)));
    tasks.spawn(operate(4, Some(3)));
    let stream = aiter_until_error(tasks);
    tokio::pin!(stream);
    while let Some(r) = stream.next().await {
        // let mut stream = aiter_until_error(tasks);
        // while let Some(r) = stream.next().await {
        match r {
            Ok(x) => println!("Got: {x}"),
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(())
}

fn aiter_until_error<T: 'static, E: 'static>(
    mut tasks: JoinSet<Result<T, E>>,
) -> impl Stream<Item = Result<T, E>> {
    stream! {
        while let Some(r) = tasks.join_next().await {
            match r {
                Ok(Ok(r)) => yield Ok(r),
                Ok(Err(e)) => {
                    tasks.shutdown().await;
                    yield Err(e);
                    break;
                },
                Err(e) => {
                    println!("Error: {e}");
                    tasks.shutdown().await;
                    break;
                }
            }
        }
    }
}

// copy from https://users.rust-lang.org/t/pinning-issues-when-creating-a-stream-from-tokios-joinset/78519
// join_all and try_join_all, as well as more versatile FuturesOrdered and FuturesUnordered utilities from the same crate futures, are executed as a single task. This is probably fine if the constituent futures are not often concurrently ready to perform work, but if you want to make use of CPU parallelism with the multi-threaded runtime, consider spawning the individual futures as separate tasks and waiting on the tasks to finish.

// see https://stackoverflow.com/questions/63589668/how-to-tokiojoin-multiple-tasks