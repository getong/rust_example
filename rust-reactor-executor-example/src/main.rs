use lazy_static::lazy_static;
//use rand::prelude::*;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::channel;
use std::sync::Mutex;

mod executor;
mod poll;
mod reactor;

type EventId = usize;

lazy_static! {
    static ref EXECUTOR: Mutex<executor::Executor> = Mutex::new(executor::Executor::new());
    static ref REACTOR: Mutex<reactor::Reactor> = Mutex::new(reactor::Reactor::new());
    static ref CONTEXTS: Mutex<HashMap<EventId, RequestContext>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
pub struct RequestContext {
    pub stream: TcpStream,
    pub content_length: usize,
    pub buf: Vec<u8>,
}

impl RequestContext {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            content_length: 0,
        }
    }

    fn read_cb(&mut self, event_id: EventId, exec: &mut executor::Executor) -> io::Result<()> {
        let mut buf = [0u8; 4096];
        match self.stream.read(&mut buf) {
            Ok(_) => {
                if let Ok(data) = std::str::from_utf8(&buf) {
                    self.parse_and_set_content_length(data);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => {
                return Err(e);
            }
        };
        self.buf.extend_from_slice(&buf);
        if self.buf.len() >= self.content_length {
            REACTOR
                .lock()
                .expect("can get reactor lock")
                .write_interest(self.stream.as_raw_fd(), event_id)
                .expect("can set write interest");

            write_cb(exec, event_id);
        } else {
            REACTOR
                .lock()
                .expect("can get reactor lock")
                .read_interest(self.stream.as_raw_fd(), event_id)
                .expect("can set write interest");
            read_cb(exec, event_id);
        }
        Ok(())
    }

    fn parse_and_set_content_length(&mut self, data: &str) {
        if data.contains("HTTP") {
            if let Some(content_length) = data
                .lines()
                .find(|l| l.to_lowercase().starts_with("content-length: "))
            {
                if let Some(len) = content_length
                    .to_lowercase()
                    .strip_prefix("content-length: ")
                {
                    self.content_length = len.parse::<usize>().expect("content-length is valid");
                    println!("set content length: {} bytes", self.content_length);
                }
            }
        }
    }

    fn write_cb(&mut self, event_id: EventId) -> io::Result<()> {
        println!("in write event of stream with event id: {}", event_id);
        match self.stream.write(HTTP_RESP) {
            Ok(_) => println!("answered from request {}", event_id),
            Err(e) => eprintln!("could not answer to request {}, {}", event_id, e),
        };
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .expect("can close a stream");

        REACTOR
            .lock()
            .expect("can get reactor lock")
            .close(self.stream.as_raw_fd())
            .expect("can close fd and clean up reactor");

        Ok(())
    }
}

const HTTP_RESP: &[u8] = br#"HTTP/1.1 200 OK
content-type: text/html
content-length: 5

Hello"#;

fn main() -> io::Result<()> {
    let listener_event_id = 100;
    let listener = TcpListener::bind("127.0.0.1:8000")?;
    listener.set_nonblocking(true)?;
    let listener_fd = listener.as_raw_fd();

    let (sender, receiver) = channel();

    match REACTOR.lock() {
        Ok(mut re) => re.run(sender),
        Err(e) => panic!("error running reactor, {}", e),
    };

    REACTOR
        .lock()
        .expect("can get reactor lock")
        .read_interest(listener_fd, listener_event_id)?;

    listener_cb(listener, listener_event_id);

    while let Ok(event_id) = receiver.recv() {
        EXECUTOR
            .lock()
            .expect("can get an executor lock")
            .run(event_id);
    }

    Ok(())
}

fn listener_cb(listener: TcpListener, event_id: EventId) {
    let mut exec_lock = EXECUTOR.lock().expect("can get executor lock");
    exec_lock.await_keep(event_id, move |exec| {
        match listener.accept() {
            Ok((stream, addr)) => {
                let event_id: EventId = rand::random::<EventId>();
                stream.set_nonblocking(true).expect("nonblocking works");
                println!(
                    "new client: {}, event_id: {}, raw fd: {}",
                    addr,
                    event_id,
                    stream.as_raw_fd()
                );
                REACTOR
                    .lock()
                    .expect("can get reactor lock")
                    .read_interest(stream.as_raw_fd(), event_id)
                    .expect("can set read interest");
                CONTEXTS
                    .lock()
                    .expect("can lock request contests")
                    .insert(event_id, RequestContext::new(stream));
                read_cb(exec, event_id);
            }
            Err(e) => eprintln!("couldn't accept: {}", e),
        };
        REACTOR
            .lock()
            .expect("can get reactor lock")
            .read_interest(listener.as_raw_fd(), event_id)
            .expect("re-register works");
    });
    drop(exec_lock);
}

fn read_cb(exec: &mut executor::Executor, event_id: EventId) {
    exec.await_once(event_id, move |write_exec| {
        if let Some(ctx) = CONTEXTS
            .lock()
            .expect("can lock request_contexts")
            .get_mut(&event_id)
        {
            ctx.read_cb(event_id, write_exec)
                .expect("read callback works");
        }
    });
}

fn write_cb(exec: &mut executor::Executor, event_id: EventId) {
    exec.await_once(event_id, move |_| {
        if let Some(ctx) = CONTEXTS
            .lock()
            .expect("can lock request_contexts")
            .get_mut(&event_id)
        {
            ctx.write_cb(event_id).expect("write callback works");
        }
        CONTEXTS
            .lock()
            .expect("can lock request contexts")
            .remove(&event_id);
    });
}
