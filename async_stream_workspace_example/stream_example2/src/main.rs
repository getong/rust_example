use futures::stream::{self, StreamExt};

#[tokio::main]
async fn main() {
    let stream = stream::iter(vec![17, 19]);
    assert_eq!(vec![17, 19], stream.collect::<Vec<i32>>().await);

    // From the zeroth to the third power of two:
    let mut curr = 1;
    let mut pow2 = stream::repeat_with(|| {
        let tmp = curr;
        curr *= 2;
        tmp
    });
    assert_eq!(Some(1), pow2.next().await);
    assert_eq!(Some(2), pow2.next().await);
    assert_eq!(Some(4), pow2.next().await);
    assert_eq!(Some(8), pow2.next().await);

    let state = (true, true, true);
    let stream = stream::unfold(state, |state| async move {
        match state {
            (true, phase2, phase3) => {
                // do some stuff for phase 1
                let item = async { 1 }.await;
                Some((item, (false, phase2, phase3)))
            }
            (_phase1, true, phase3) => {
                // do some stuff for phase 2
                let item = async { 2 }.await;
                Some((item, (false, false, phase3)))
            }
            (_phase1, _phase2, true) => {
                // do some stuff for phase 3
                let item = async { 3 }.await;
                Some((item, (false, false, false)))
            }
            _ => None,
        }
    });
    tokio::pin!(stream);
    assert_eq!(Some(1), stream.next().await);
    assert_eq!(Some(2), stream.next().await);
    assert_eq!(Some(3), stream.next().await);
    assert_eq!(None, stream.next().await);

    let stream = stream::iter(vec![1, 2, 3]);
    let mut stream = stream.inspect(|val| println!("{}", val));
    assert_eq!(stream.next().await, Some(1));
    // will print also in the console "1"

    let stream = async_stream::stream! {
        // yield async { 1 }.await;
        // yield async { 2 }.await;
        // yield async { 3 }.await;
        for i in 1..=3 {
            yield i;
        }
    };
    tokio::pin!(stream);
    assert_eq!(Some(1), stream.next().await);
    assert_eq!(Some(2), stream.next().await);
    assert_eq!(Some(3), stream.next().await);
    assert_eq!(None, stream.next().await);
}

// copy from https://www.qovery.com/blog/a-guided-tour-of-streams-in-rust
