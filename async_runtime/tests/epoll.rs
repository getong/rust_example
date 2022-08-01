#[macro_use]
extern crate async_runtime;
use std::os::unix::prelude::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[test]
fn create_epoll_instance() {
    let epoll_fd = syscall!(epoll_create(1)).unwrap();
    syscall!(close(epoll_fd)).unwrap();
}

#[test]
fn epoll_read() {
    let epoll_fd = syscall!(epoll_create(1)).unwrap();

    let mut stream = TcpStream::connect("127.0.0.1:8080").unwrap();

    let mut event = libc::epoll_event {
        events: (0i32 | libc::EPOLLIN) as u32,
        u64: 0xfeedbeaf,
    };

    syscall!(epoll_ctl(
        epoll_fd,
        libc::EPOLL_CTL_ADD,
        stream.as_raw_fd(),
        &mut event
    ))
    .unwrap();

    let mut events: Vec<libc::epoll_event> = Vec::with_capacity(10);

    // send request, and use epoll to wait for response event
    println!("written bytes: {}", stream.write(b"Delay 1").unwrap());

    // blocking
    loop {
        
        let res = syscall!(epoll_wait(epoll_fd, events.as_mut_ptr(), 10, 2000) ).unwrap();
        if res > 0 {
            for i in 0..res as usize {
                println!("current fd: {i}");
                let ready_event = unsafe { *events.as_ptr().add(i) };
                let event_mask = ready_event.events;
                let data = ready_event.u64;
                println!("event mask: {}, data: {:#x}", event_mask, data);
                let mut buf = [0; 1500];
                println!("read bytes: {}", stream.read(&mut buf).unwrap());
            }
        } else {
            println!("timeout, return {}", res);
            break;
        }
    }

    stream.shutdown(std::net::Shutdown::Both).unwrap();
    syscall!(close(epoll_fd)).unwrap();
}

