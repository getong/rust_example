use futures::channel::mpsc;
use futures::future::{Fuse, FusedFuture, FutureExt};
use futures::select;
use futures::stream::StreamExt;
use futures::pin_mut;

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let (sender, mut reader) = mpsc::unbounded();

    // Send a few messages into the stream
    sender.unbounded_send(()).unwrap();
    sender.unbounded_send(()).unwrap();
    drop(sender);

    // Use `Fuse::terminated()` to create an already-terminated future
    // which may be instantiated later.
    let foo_printer = Fuse::terminated();
    pin_mut!(foo_printer);

    loop {
        select! {
            _ = foo_printer => {
                println!("foo print actually");
            },
            () = reader.select_next_some() => {
                if !foo_printer.is_terminated() {
                    println!("Foo is already being printed!");
                } else {
                    foo_printer.set(async {
                        // do some other async operations
                        println!("Printing foo from `foo_printer` future");
                    }.fuse());
                }
            },
            complete => break, // `foo_printer` is terminated and the stream is done
        }
    }
}
