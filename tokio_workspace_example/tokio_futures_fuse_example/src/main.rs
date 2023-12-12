use futures::future::{Fuse, FusedFuture, FutureExt};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");
  let (sender, mut reader) = mpsc::unbounded_channel();

  // Send a few messages into the stream
  sender.send(()).unwrap();
  sender.send(()).unwrap();
  drop(sender);

  // Use `Fuse::terminated()` to create an already-terminated future
  // which may be instantiated later.
  let foo_printer = Fuse::terminated();
  tokio::pin!(foo_printer);

  loop {
    tokio::select! {
        _ = foo_printer.as_mut().fuse() => {
            println!("foo print actually");
            break;
        },
        Some(()) = reader.recv() => {
            if !foo_printer.is_terminated() {
                println!("Foo is already being printed!");
            } else {
                foo_printer.set(async {
                    // do some other async operations
                    println!("Printing foo from `foo_printer` future");
                }.fuse());
            }

        },
        else => {
            println!("other");
            break;
        },

    }
  }
}
