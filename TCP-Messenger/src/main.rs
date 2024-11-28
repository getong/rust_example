// use std::io::stdin;
use std::sync::mpsc;
use std::{
  sync::{Arc, Mutex},
  thread, time,
};

use clap::{Arg, Command};
use messenger::{server::Server, user::User};

fn main() {
  let app = Command::new("messenger")
    .about("Simple Messenger")
    .arg(Arg::new("mode").required(true))
    .arg(Arg::new("server").required(false))
    .get_matches();

  let mode = app.value_of("mode").unwrap();
  let ip_port = app.value_of("server").unwrap();
  if mode == "server" {
    let mut handler = Server::new(ip_port);
    handler.listen();
    // loop {
    //     thread::sleep(time::Duration::from_secs(5));
    //     let mut clients = handler.clients.lock().unwrap();
    //     println!("{:?}",clients);

    //     for v in clients.iter_mut() {
    //         let val: Vec<u8> = v.read_stream();
    //         println!("read stream: {:?}",std::str::from_utf8(&val[..]));
    //         v.write_stream("Micheal here!");
    //     }
    // }
    // handler.broadcast_client();
    thread::sleep(time::Duration::from_secs(500));
  } else if mode == "client" {
    let mut user = User::new(ip_port).unwrap();
    let messages: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(vec![]));
    user.read_stream(&messages);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || loop {
      let mut line = String::new();

      let _v = std::io::stdin().read_line(&mut line).unwrap();
      tx.send(line).unwrap();
    });
    let mut count = 0;
    loop {
      {
        // println!("{:?}", messages);
        let messages = &messages;
        let lock = messages.lock().unwrap();
        for i in count .. lock.len() {
          let s = std::str::from_utf8(&lock[i]).unwrap();
          if s.len() != 0 {
            println!("> {}", s);
          }
          count += 1;
        }
      }
      // let mut line = String::new();
      // let v = std::io::stdin().read_line(&mut line).unwrap();
      if let Ok(line) = rx.recv_timeout(time::Duration::from_millis(50)) {
        user.write_stream(&line);
      }
    }
  } else {
    eprintln!(
      "Usage: messenger <server / client>\nEg:\n\tmessenger server\n\tmessenger client \
       127.0.0.1:9000"
    );
    return;
  }
}
