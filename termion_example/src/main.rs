use std::io::stdout;
use std::mem::drop;
use std::process::exit;
use std::time::Duration;

use futures::{future::Either, StreamExt};
use tokio::{io::stdin, time::interval};

use termion::{event::Key, raw::IntoRawMode};
use termion_input_tokio::TermReadAsync;
use tokio_stream::wrappers::IntervalStream;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let raw_term = stdout().into_raw_mode()?;

    let input = stdin().keys_stream().map(Either::Right);
    let ticks = IntervalStream::new(interval(Duration::from_secs(3))).map(Either::Left);

    let events = futures::stream::select(ticks, input);

    events
        .fold(raw_term, |raw_term, it| {
            println!("Event: {:?}\r", it);
            match it {
                Either::Right(Ok(Key::Esc))
                | Either::Right(Ok(Key::Char('q')))
                | Either::Right(Ok(Key::Ctrl('c')))
                | Either::Right(Ok(Key::Ctrl('b'))) => {
                    drop(raw_term);
                    exit(0);
                }
                _ => (),
            }
            async { raw_term }
        })
        .await;
    Ok(())
}
