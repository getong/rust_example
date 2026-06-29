use std::time::Duration;

use kameo::{
  actor::ActorRef,
  error::Infallible,
  prelude::{Actor, Context, Message, Spawn},
};

// 1. 定义 Actor
struct MyActor;

impl Actor for MyActor {
  type Args = Self;
  type Error = Infallible;

  // 当 Actor 启动时自动触发
  async fn on_start(actor: Self::Args, actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
    println!("Actor 已启动！");

    // 异步延迟给自己发消息（不会阻塞 Actor 自身的消息循环）
    send_self_after(actor_ref, Duration::from_secs(3));

    Ok(actor)
  }
}

// 2. 定义内部消息
struct LoopMessage;

impl Message<LoopMessage> for MyActor {
  type Reply = ();

  async fn handle(
    &mut self,
    _msg: LoopMessage,
    ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    println!("收到来自自己的延迟消息！");

    // 如果你想实现“每隔3秒定期执行”，可以在这里继续给自己发消息
    send_self_after(ctx.actor_ref().clone(), Duration::from_secs(3));
  }
}

fn send_self_after(actor_ref: ActorRef<MyActor>, delay: Duration) {
  tokio::spawn(async move {
    tokio::time::sleep(delay).await;
    let _ = actor_ref.tell(LoopMessage).await;
  });
}

// 3. 运行测试
#[tokio::main]
async fn main() {
  // 启动 Actor
  let actor_ref = MyActor::spawn(MyActor);

  // 让主线程等待，以便观察 Actor 的延迟打印
  tokio::time::sleep(Duration::from_secs(10)).await;
  let _ = actor_ref.stop_gracefully().await;
}
