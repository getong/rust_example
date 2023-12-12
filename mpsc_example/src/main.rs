use std::sync::mpsc;

fn main() {
  println!("Hello, world!");

  let (snd, rcv) = mpsc::channel();
  snd.send("Wubble wubble foo").unwrap();
  println!("Message is: {}", rcv.recv().unwrap());
}
