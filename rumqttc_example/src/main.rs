use std::{thread, time::Duration};

use rumqttc::{Client, MqttOptions, QoS};

fn main() {
  let mut mqttoptions = MqttOptions::new("NAME", "YOUR BROKER", 1883);
  mqttoptions.set_keep_alive(Duration::from_secs(5));
  let (mut client, mut connection) = Client::new(mqttoptions, 10);
  client.subscribe("demo/mqtt", QoS::AtMostOnce).unwrap();
  thread::spawn(move || {
    for i in 0 .. 10 {
      client
        .publish("demo/mqtt", QoS::AtLeastOnce, false, vec![i; i as usize])
        .unwrap();
      thread::sleep(Duration::from_millis(100));
    }
  });
  for (_i, message) in connection.iter().enumerate() {
    println!("Message = {:?}", message);
  }
}
