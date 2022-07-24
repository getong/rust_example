use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use tokio::time::sleep;

pub struct Parker {
    waker: Waker,
    unparked: Arc<AtomicBool>,
}

impl Parker {
    pub fn unpark(self) {
        // Order should be kept, or we may wake and (falsely) reveal we haven't been
        // unparked, then never wake again.
        self.unparked.store(true, Ordering::SeqCst);
        self.waker.wake();
    }
}

// You can get rid of this `Unpin` bound, if you really want
pub async fn park(callback: impl FnOnce(Parker) + Unpin) {
    enum Park<F> {
        FirstTime { callback: F },
        SecondTime { unparked: Arc<AtomicBool> },
    }

    impl<F: FnOnce(Parker) + Unpin> Future for Park<F> {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Self::SecondTime { unparked } = &*self {
                return if unparked.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                };
            }

            let unparked = Arc::new(AtomicBool::new(false));
            let callback = match std::mem::replace(
                &mut *self,
                Self::SecondTime {
                    unparked: Arc::clone(&unparked),
                },
            ) {
                Self::FirstTime { callback } => callback,
                Self::SecondTime { .. } => unreachable!(),
            };
            callback(Parker {
                waker: cx.waker().clone(),
                unparked,
            });
            Poll::Pending
        }
    }

    Park::FirstTime { callback }.await
}

async fn race(f1: impl Future<Output = ()>, f2: impl Future<Output = ()>) {
    tokio::select! {
        () = f1 => {}
        () = f2 => {}
    }
}

async fn resume_immediately() {
    park(|p| p.unpark()).await;
    println!("resume_immediately()");
}

async fn never_resume() {
    park(|_| {}).await;
    println!("never_resume()");
}

async fn after_timeout() {
    sleep(Duration::from_millis(300)).await;
    println!("after_timeout()");
}

async fn park_for_100ms() {
    park(|p| {
        tokio::spawn(async {
            sleep(Duration::from_millis(100)).await;
            p.unpark()
        });
    })
    .await;
    println!("park_for_100ms()");
}

async fn park_for_500ms() {
    park(|p| {
        tokio::spawn(async {
            sleep(Duration::from_millis(500)).await;
            p.unpark()
        });
    })
    .await;
    println!("park_for_500ms()");
}

#[tokio::main]
async fn main() {
    race(after_timeout(), resume_immediately()).await;
    race(after_timeout(), never_resume()).await;

    race(after_timeout(), park_for_100ms()).await;
    race(after_timeout(), park_for_500ms()).await;
}
