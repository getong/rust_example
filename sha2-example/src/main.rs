extern crate sha2;
use sha2::{Digest, Sha256};
use std::fs::ReadDir;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::thread;
use std::{env, fs};

fn sock_server(mut listener: UnixStream) {
    loop {
        let mut response = String::new();
        let _length = listener.read_to_string(&mut response).unwrap();
        println!("{}", response);
    }
}

fn get_files(path: &String) -> ReadDir {
    fs::read_dir(&path).unwrap()
}

fn read_file(filename: &fs::DirEntry) -> Result<String, ()> {
    if !filename.path().is_dir() {
        let contents = match fs::read_to_string(filename.path()) {
            Ok(contents) => contents,
            Err(_why) => panic!("{:?}", filename.path()),
        };
        Ok(contents)
    } else {
        Err(())
    }
}

fn main() -> Result<(), ()> {
    let current_dir = String::from(env::current_dir().unwrap().to_str().unwrap());
    let (side1, mut side2) = match UnixStream::pair() {
        Ok((side1, side2)) => (side1, side2),
        Err(e) => {
            println!("Couldn't create a pair of sockets: {:?}", e);
            std::process::exit(-1);
        }
    };
    let _serv_handle = thread::spawn(|| sock_server(side1));
    for file in get_files(&current_dir) {
        let entry = file.unwrap();
        if let Ok(file) = read_file(&entry) {
            let msg = format!(
                "{} : {:x}\n",
                entry.path().to_str().unwrap(),
                Sha256::digest(file.as_bytes())
            );
            side2.write_all(&msg.into_bytes()).unwrap();
        }
    }
    Ok(())
}
