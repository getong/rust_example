// copy from https://matthewtejo.substack.com/p/building-robust-server-with-async

use rocket::get;
use rocket::routes;
use std::str::from_utf8;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::runtime::Handle;

async fn stream_handler(mut t: TcpStream) {
  let mut buf = [0; 1024];

  loop {
    t.readable().await.unwrap();
    let n = match t.read(&mut buf).await {
      // socket closed
      Ok(n) if n == 0 => return,
      Ok(n) => n,
      Err(e) => {
        eprintln!("failed to read from socket; err = {:?}", e);
        return;
      }
    };
    let answer = from_utf8(&buf[0..n]).expect("some utf8 issue");
    t.writable().await.unwrap();
    _ = t.try_write(format!("Back to you! {}", answer).as_bytes());
  }
}

#[get("/admin")]
fn http_admin() -> &'static str {
  "<html><h1>Hello from admin!</h1></html>"
}

fn start_http(rt: &Handle) {
  let figment = rocket::Config::figment()
    .merge(("port", 9999))
    .merge(("shutdown.ctrlc", false));

  let rocket = rocket::custom(figment).mount("/", routes![http_admin]);

  let http_server = rocket.launch();
  rt.spawn(http_server);
}

fn start_server() {
  //... below runtime code

  let acceptor_runtime = Builder::new_multi_thread()
    .worker_threads(1)
    .thread_name("acceptor-pool")
    .thread_stack_size(3 * 1024 * 1024)
    .enable_time()
    .enable_io()
    .build()
    .unwrap();

  let request_runtime = Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("request-pool")
    .thread_stack_size(3 * 1024 * 1024)
    .enable_time()
    .enable_io()
    .build()
    .unwrap();

  let utility_runtime = Builder::new_multi_thread()
    .worker_threads(1)
    .thread_name("utility-pool")
    .thread_stack_size(3 * 1024 * 1024)
    .enable_time()
    .enable_io()
    .build()
    .unwrap();
  acceptor_runtime.block_on(async {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    loop {
      let (socket, _) = listener.accept().await.unwrap();
      let _g = request_runtime.enter();
      request_runtime.spawn(stream_handler(socket));
    }
  });

  start_http(utility_runtime.handle());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // do stuff like parse cli args

  // build runtimes

  start_server();
  return Ok(());
}
