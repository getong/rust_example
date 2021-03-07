use std::env;
use std::net::ToSocketAddrs;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Please provide only one host name");
        std::process::exit(1);
    } else {
        // 80 is the host default port
        let addr_str = args[1].to_owned() + ":80";
        let addresses = addr_str.to_socket_addrs().unwrap();
        for address in addresses {
            println!("{}", address);
        }
    }
}
