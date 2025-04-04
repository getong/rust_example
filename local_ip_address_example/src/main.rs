use local_ip_address::{list_afinet_netifas, local_ip};

fn main() {
  let my_local_ip = local_ip().unwrap();

  println!("\nThis is my local IP address: {:?}\n", my_local_ip);

  let network_interfaces = list_afinet_netifas().unwrap();

  for (name, ip) in network_interfaces.iter() {
    println!("{}:\t{:?}", name, ip);
  }
}
