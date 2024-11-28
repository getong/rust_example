use std::{collections::VecDeque, sync::Arc};

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpStream,
  sync::Mutex,
  task::JoinHandle,
};

pub struct HeaderCrypt {}
impl HeaderCrypt {
  pub fn encrypt(&mut self, _data: &[u8]) -> Vec<u8> {
    return vec![];
  }
  pub fn decrypt(&mut self, _data: &[u8]) -> Vec<u8> {
    return vec![];
  }
}

pub struct Session {
  pub header_crypt: Option<HeaderCrypt>,
}
impl Session {
  pub fn new() -> Self {
    Self { header_crypt: None }
  }
}

pub struct Client {
  stream: Arc<Mutex<Option<TcpStream>>>,
  queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
  session: Arc<Mutex<Session>>,
}

impl Client {
  pub fn new() -> Self {
    Self {
      stream: Arc::new(Mutex::new(None)),
      queue: Arc::new(Mutex::new(VecDeque::new())),
      session: Arc::new(Mutex::new(Session::new())),
    }
  }

  pub async fn connect(&mut self, host: &str, port: i16) {
    let addr = format!("{}:{}", host, port);
    match TcpStream::connect(&addr).await {
      Ok(stream) => {
        self.stream = Arc::new(Mutex::new(Some(stream)));
        println!("Connected to {}", addr);
      }
      _ => {
        panic!("Cannot connect");
      }
    }
  }

  pub async fn handle_connection(&mut self) {
    loop {
      let _ = self.handle_read();
      let _ = self.handle_queue();
      let _ = self.handle_write();
    }
  }

  async fn handle_queue(&mut self) -> JoinHandle<()> {
    let queue = Arc::clone(&self.queue);

    tokio::spawn(async move {
      match queue.lock().await.pop_front() {
        Some(_packet) => {}
        _ => {}
      }
    })
  }

  async fn handle_write(&mut self) -> JoinHandle<()> {
    let stream = Arc::clone(&self.stream);
    let session = Arc::clone(&self.session);
    let queue = Arc::clone(&self.queue);

    tokio::spawn(async move {
      match stream.lock().await.as_mut() {
        Some(stream) => {
          let packet = queue.lock().await.pop_front().unwrap();

          let packet = match session.lock().await.header_crypt.as_mut() {
            Some(header_crypt) => header_crypt.encrypt(&packet),
            _ => packet,
          };

          stream.write(&packet).await.unwrap();
          stream.flush().await.unwrap();
        }
        _ => {}
      };
    })
  }

  async fn handle_read(&mut self) -> JoinHandle<()> {
    let queue = Arc::clone(&self.queue);
    let stream = Arc::clone(&self.stream);
    let session = Arc::clone(&self.session);

    tokio::spawn(async move {
      match stream.lock().await.as_mut() {
        Some(stream) => {
          let mut buffer = [0u8; 4096];

          match stream.read(&mut buffer).await {
            Ok(bytes_count) => {
              let raw_data = match session.lock().await.header_crypt.as_mut() {
                Some(header_crypt) => header_crypt.decrypt(&buffer[.. bytes_count]),
                _ => buffer[.. bytes_count].to_vec(),
              };

              queue.lock().await.push_back(raw_data);
            }
            _ => {}
          };
        }
        _ => {}
      };
    })
  }
}

#[tokio::main]
async fn main() {
  let mut client = Client::new();
  client.connect("127.0.0.1", 3724).await;
  client.handle_connection().await;
}
