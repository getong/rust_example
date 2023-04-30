use std::{
    sync::mpsc::{channel, Receiver, Sender},
    thread,
};

trait Bar {
    fn bar(&self);
}

struct Foo {
    foo: i32,
}

impl Bar for Foo {
    fn bar(&self) {
        println!("foo: {}", self.foo);
    }
}

fn main() {
    // let foo = Box::new(Foo { foo: 1 }) as Box<dyn Bar>;

    // let (tx, rx): (Sender<Box<dyn Bar>>, Receiver<Box<dyn Bar>>) = channel();
    let foo = Box::new(Foo { foo: 1 }) as Box<dyn Bar + Send>;

    let (tx, rx): (Sender<Box<dyn Bar + Send>>, Receiver<Box<dyn Bar + Send>>) = channel();

    thread::spawn(move || {
        tx.send(foo).unwrap();
    });

    let sent = rx.recv().unwrap();

    sent.bar();
}