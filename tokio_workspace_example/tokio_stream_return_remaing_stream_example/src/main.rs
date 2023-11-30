use futures::ready;
use futures::{
    stream::{FuturesUnordered, StreamFuture},
    Stream, StreamExt,
};

use std::{
    pin::Pin,
    task::{Context, Poll},
};

// Define the InQReader struct.
pub struct InQReader<St>
where
    St: Stream + Unpin,
{
    inner: FuturesUnordered<StreamFuture<St>>,
}

impl<St: Stream + Unpin> InQReader<St> {
    // Constructs a new, empty InQReader.
    pub fn new() -> Self {
        Self {
            inner: FuturesUnordered::new(),
        }
    }

    // Adds a stream to the InQReader.
    pub fn push(&mut self, stream: St) {
        self.inner.push(stream.into_future());
    }
}

impl<St: Stream + Unpin> Stream for InQReader<St> {
    type Item = (St::Item, St);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.inner.poll_next_unpin(cx)) {
                Some((Some(item), remaining)) => {
                    return Poll::Ready(Some((item, remaining)));
                }
                Some((None, _)) => {
                    // `FuturesUnordered` thinks it isn't terminated
                    // because it yielded a Some.
                    // We do not return, but poll `FuturesUnordered`
                    // in the next loop iteration.
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

// Example usage.
#[tokio::main]
async fn main() {
    let stream1 = futures::stream::iter(vec![1, 2, 3]);
    let stream2 = futures::stream::iter(vec![4, 5, 6]);

    let mut reader = InQReader::new();
    reader.push(stream1);
    reader.push(stream2);

    while let Some((item, remaining)) = reader.next().await {
        println!("Item: {}", item);
        reader.push(remaining);
    }
}
