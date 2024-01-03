use std::process::Command;
use std::process::Stdio;

fn main() {
  // println!("Hello, world!");
  pipe();
}

pub fn pipe() {
  // 创建两个子进程，一个作为生产者，一个作为消费者

  // 生产者进程
  let producer = Command::new("echo")
    .arg("Hello, Rust!")
    .stdout(Stdio::piped())
    .spawn()
    .expect("Failed to start producer command");

  // 消费者进程
  let consumer = Command::new("grep")
    .arg("Rust")
    .stdin(producer.stdout.unwrap())
    .output()
    .expect("Failed to start consumer command");

  // 获取消费者的输出
  let output = String::from_utf8_lossy(&consumer.stdout);
  println!("Output: {:?}", output);
}
