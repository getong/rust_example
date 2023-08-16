use std::pin::Pin;

use pin_project_lite::pin_project;

pin_project! {
    struct Struct<T, U> {
        #[pin]
        pinned: T,
        unpinned: U,
    }
}

impl<T, U> Struct<T, U> {
    fn method(self: Pin<&mut Self>) {
        let this = self.project();
        let _: Pin<&mut T> = this.pinned; // Pinned reference to the field
        let _: &mut U = this.unpinned; // Normal reference to the field
    }
}

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let mut my_struct = Struct {
        pinned: 10,
        unpinned: &123,
    };

    Pin::new(&mut my_struct).method();
}
