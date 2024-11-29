// use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
// use std::cell::RefCell;
// use std::fs::File;
// use std::marker::Sized;
use std::{
  net::TcpListener,
  sync::{Arc, Mutex},
  thread, time,
};

// use std::io::{self, Read, Write};
use crate::client::Client;
// use crate::server;

pub struct Server {
  pub listener: Arc<TcpListener>,
  pub clients: Arc<Mutex<Vec<Arc<Mutex<Client>>>>>,
}

impl Server {
  pub fn new(ip_port: &str) -> Self {
    let listener = TcpListener::bind(ip_port).unwrap();

    Server {
      listener: Arc::new(listener),
      clients: Arc::new(Mutex::new(vec![])),
    }
  }

  pub fn listen(&mut self) {
    let temp_listener = Arc::clone(&self.listener);
    let temp_clients = Arc::clone(&self.clients);
    thread::spawn(move || {
      for stream in temp_listener.incoming() {
        match stream {
          Ok(s) => {
            println!("{:?}", s.peer_addr());

            let client = Client::new(s);
            let mut d = temp_clients.lock().unwrap();

            let temp = Arc::new(Mutex::new(client));
            let _c = temp.clone();
            d.push(temp.clone());

            if d.len() == 2 {
              let sender = Arc::clone(&d[0]);
              let receiver = Arc::clone(&d[1]);
              thread::spawn(move || loop {
                {
                  let mut lock_sender = sender.lock().unwrap();
                  let mut lock_receiver = receiver.lock().unwrap();
                  let mut message = lock_sender.read_stream();
                  let string = std::str::from_utf8_mut(&mut message[..]).unwrap();
                  lock_receiver.write_stream(string);
                }
                thread::sleep(time::Duration::from_millis(200));
              });
              let sender = Arc::clone(&d[1]);
              let receiver = Arc::clone(&d[0]);
              thread::spawn(move || loop {
                thread::sleep(time::Duration::from_millis(250));
                {
                  let mut lock_sender = sender.lock().unwrap();
                  let mut lock_receiver = receiver.lock().unwrap();
                  let mut message = lock_sender.read_stream();
                  let string = std::str::from_utf8_mut(&mut message[..]).unwrap();
                  lock_receiver.write_stream(string);
                }
              });
            }

            // thread::spawn(move ||{
            //         thread::sleep(time::Duration::from_secs(2));
            //             println!("Handling client in a thread");
            //             {
            //             let mut st = c.lock().unwrap();
            //             // let val: Vec<u8> = st.read_stream();
            //             // let command = std::str::from_utf8(&val[..]).unwrap();
            //             // println!("read stream: {:?}",command);
            //       //      if command
            //             }
            //             //st.write_stream("Micheal here!");
            //             thread::sleep(time::Duration::from_secs(2));
            // });
          }
          _ => {
            eprintln!("Error processing stream")
          }
        }
      }
    });
  }

  pub fn broadcast_client(&mut self) {
    let clients = Arc::clone(&mut self.clients);

    thread::spawn(move || loop {
      {
        let mut values = clients.lock().unwrap();
        let temp = format!("{:?}", values);
        for gaurd in values.iter_mut() {
          let mut clis = gaurd.lock().unwrap();
          clis.write_stream(&temp);
        }
      }
      thread::sleep(time::Duration::from_secs(10));
    });
  }
}

#[cfg(test)]
mod test {
  // use super::*;

  // #[test]
  // fn write_to_stream() {
  //     let mut handler = Server::new("127.0.0.1:9000");
  //     handler.listen();
  //     thread::spawn(move || {
  //         thread::sleep(time::Duration::from_millis(500));
  //         let mut stream = TcpStream::connect("127.0.0.1:9000").expect("couldnt connect to
  // server");         stream.set_nonblocking(true).expect("nonblocking failed");

  //         stream.write(&"test".as_bytes()).unwrap();
  //         println!("client 1 is connected");

  //         // let mut buf = vec![];
  //         // loop {
  //         // println!("client 1 is going to read its stream");
  //         //     match stream.read_to_end(&mut buf) {
  //         //         Ok(_) => {
  //         //             println!("{:?}", std::str::from_utf8_mut(&mut buf[..]));
  //         //             thread::sleep(time::Duration::from_secs(5));
  //         //         },
  //         //         _ => eprintln!("io error")
  //         //     }
  //         //     thread::sleep(time::Duration::from_secs(40));
  //         // }
  //     });
  //     thread::spawn(move || {
  //         thread::sleep(time::Duration::from_millis(700));
  //         let mut stream = TcpStream::connect("127.0.0.1:9000").expect("couldnt connect to
  // server");         stream.set_nonblocking(true).expect("nonblocking failed");
  //         stream.write(&"how are you?".as_bytes()).unwrap();
  //         println!("client 2 is connected");

  //         // let mut buf = vec![];

  //         // loop {
  //         //     println!("client 2 is goinf to read tis stream");

  //         //     match stream.read_to_end(&mut buf) {
  //         //         Ok(_) => {
  //         //             println!("{:?}", std::str::from_utf8_mut(&mut buf[..]));
  //         //             thread::sleep(time::Duration::from_secs(5));
  //         //         },
  //         //         _ => eprintln!("io error")
  //         //     }
  //         //     thread::sleep(time::Duration::from_secs(40));
  //         // }
  //     });

  //     // loop {
  //         thread::sleep(time::Duration::from_secs(5));
  //         let mut clients = handler.clients.lock().unwrap();
  //         println!("{:?}",clients);

  //         for v in clients.iter_mut() {
  //             let val: Vec<u8> = v.read_stream();
  //             println!("read stream: {:?}",std::str::from_utf8(&val[..]));
  //             v.write_stream("Micheal here!");
  //         }
  //         println!("here written to both clients")
  //     // }

  // }

  // fn pass_message_between_client() {
  //     let sender = Client::new(stream: TcpStream)
  // }
}
