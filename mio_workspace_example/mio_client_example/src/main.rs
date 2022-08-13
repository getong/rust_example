use mio::net::TcpStream;
use mio::{Events, Poll as Mio_Poll, Interest, Token};
use std::io::Read;
/*
So i have create a Executor, 
    It has a Events register (task themselves will register or unregister)

    we poll for I/O events in a loop

*/

struct Executor {
    clients: Vec<TcpStream>,
    poll: Mio_Poll,
    events: Events,
    client_total: Vec<u32>
}

impl Executor {
    fn new() -> Self {
        let poll = Mio_Poll::new().unwrap();
        let events = Events::with_capacity(10);
        let mut client1 = TcpStream::connect("127.0.0.1:8000".parse().unwrap()).unwrap();

        let mut client2 = TcpStream::connect("127.0.0.1:8001".parse().unwrap()).unwrap();

        let mut client3 = TcpStream::connect("127.0.0.1:8002".parse().unwrap()).unwrap();

        poll.registry().register(&mut client1, Token(1), Interest::READABLE).unwrap();

        poll.registry().register(&mut client2, Token(2), Interest::READABLE).unwrap();

        poll.registry().register(&mut client3, Token(3), Interest::READABLE).unwrap();
        Executor {
            clients: vec![client1,  client2, client3], 
            poll: poll,
            events: events,
            client_total: vec![0, 0, 0]
        }
    }

    fn poll(&mut self) {
        self.poll.poll(&mut self.events, None).unwrap();
        let mut att = 0;
        for event in &self.events {
            att += 1;
            if event.token() == Token(1) && event.is_readable() {
                let mut buf = vec![13;0];
                self.clients[0].read_exact(&mut buf).unwrap();
                self.client_total[0] += 1;
                println!("Event 1 read {} times so far.", self.client_total[0]);
            } else if event.token() == Token(2) && event.is_readable() {
                let mut buf = vec![13;0];
                self.clients[1].read_exact(&mut buf).unwrap();
                self.client_total[1] += 1;
                println!("Event 2 read {} times so far.", self.client_total[1]);
            } else if event.token() == Token(3) && event.is_readable() {
                let mut buf = vec![13;0];
                self.clients[1].read_exact(&mut buf).unwrap();
                self.client_total[2] += 1;
                println!("Event 3 read {} times so far.", self.client_total[2]);
            } 
        }
        println!("Current Poll attempt total: {}", att);
    }
}

fn main() {
    let mut executor = Executor::new();

    loop {
        if executor.client_total[0] == 20 && executor.client_total[1] == 20 && executor.client_total[2] == 20 {
            break;
        }
        executor.poll();

    }
}