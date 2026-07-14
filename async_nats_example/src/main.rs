use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // 连接到 docker 中运行的 NATS 容器 (run_nats.sh 启动, 映射到 localhost:4222)
  let client = async_nats::connect("nats://127.0.0.1:4222").await?;
  println!("已连接到 NATS: {:?}", client.connection_state());

  // 订阅主题
  let mut subscriber = client.subscribe("demo.greet").await?;

  // 发布消息
  for i in 0 .. 5 {
    let payload = format!("hello nats #{i}");
    client.publish("demo.greet", payload.into()).await?;
  }
  client.flush().await?;

  // 接收消息
  for _ in 0 .. 5 {
    if let Some(message) = subscriber.next().await {
      println!(
        "收到消息 [{}]: {}",
        message.subject,
        String::from_utf8_lossy(&message.payload)
      );
    }
  }

  // 请求/响应模式: 先启动一个响应任务
  let service_client = client.clone();
  tokio::spawn(async move {
    let mut requests = service_client.subscribe("demo.echo").await.unwrap();
    while let Some(request) = requests.next().await {
      if let Some(reply) = request.reply {
        let response = format!("echo: {}", String::from_utf8_lossy(&request.payload));
        service_client
          .publish(reply, response.into())
          .await
          .unwrap();
      }
    }
  });

  let response = client.request("demo.echo", "ping".into()).await?;
  println!("请求响应: {}", String::from_utf8_lossy(&response.payload));

  Ok(())
}
