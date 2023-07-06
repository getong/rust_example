// use tokio::stream::StreamExt;
// use tokio_stream::wrappers::Fuse;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

// use std::pin::Pin;
// use std::task::{Context, Poll};
use std::pin::Pin;
use std::task::{Context, Poll};

// a stream which alternates between Some and None
struct Alternate {
    state: i32,
}

impl Stream for Alternate {
    type Item = i32;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<i32>> {
        let val = self.state;
        self.state += 1;

        // if it's even, Some(i32), else None
        if val % 2 == 0 {
            Poll::Ready(Some(val))
        } else {
            Poll::Ready(None)
        }
    }
}

#[tokio::main]
async fn main() {
    let stream = tokio_stream::iter(vec![1, 2, 3]);

    // 使用 fuse 函数创建 Fuse<S> 包装器
    let fused_stream = stream.fuse();

    // 通过循环消费流的元素
    pin_utils::pin_mut!(fused_stream);
    while let Some(item) = fused_stream.next().await {
        println!("Received: {}", item);
    }

    let mut stream = Alternate { state: 0 };

    // the stream goes back and forth
    assert_eq!(stream.next().await, Some(0));
    assert_eq!(stream.next().await, None);
    assert_eq!(stream.next().await, Some(2));
    assert_eq!(stream.next().await, None);

    // however, once it is fused
    let mut stream = stream.fuse();

    assert_eq!(stream.next().await, Some(4));
    assert_eq!(stream.next().await, None);

    // it will always return `None` after the first time.
    assert_eq!(stream.next().await, None);
    assert_eq!(stream.next().await, None);
    assert_eq!(stream.next().await, None);
}
